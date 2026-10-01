import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_error;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as preview;
import 'package:ilikepdf/src/rust/api/pdf_security.dart' as security;
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets(
    'bridge protects and unlocks a Unicode PDF without changing the source',
    (tester) async {
      final sources = await Directory.systemTemp.createTemp(
        'ilikepdf-security-sources-',
      );
      final protectedDestination = await Directory.systemTemp.createTemp(
        'ilikepdf-protected-output-',
      );
      final unlockedDestination = await Directory.systemTemp.createTemp(
        'ilikepdf-unlocked-output-',
      );
      addTearDown(() async {
        await sources.delete(recursive: true);
        await protectedDestination.delete(recursive: true);
        await unlockedDestination.delete(recursive: true);
      });

      final source = await File(
        '${sources.path}${Platform.pathSeparator}ต้นฉบับ two pages.pdf',
      ).writeAsBytes(await _fixture('two_page.pdf').readAsBytes());
      final sourceBefore = await source.readAsBytes();
      const password = 'รหัสผ่าน Unicode 256 🔒';
      const protectedName = 'รายงาน protected.pdf';

      final sourceInfo = await security.inspectProtectPdfSource(
        sourcePath: source.path,
      );
      expect(
        sourceInfo.encryptionState,
        security.PdfEncryptionState.unencrypted,
      );
      expect(sourceInfo.pageCount, 2);
      expect(sourceInfo.defaultOutputName, 'ต้นฉบับ two pages-protected.pdf');

      final request = security.ProtectPdfRequest(
        sourcePath: source.path,
        destinationDirectory: protectedDestination.path,
        outputName: protectedName,
        password: password,
        confirmation: password,
      );
      final firstProtect = await security.protectPdf(request: request).toList();
      final secondProtect = await security
          .protectPdf(request: request)
          .toList();

      expect(firstProtect.first.stage, security.ProtectPdfStage.preparing);
      expect(
        firstProtect.map((update) => update.stage),
        containsAll([
          security.ProtectPdfStage.protecting,
          security.ProtectPdfStage.validating,
          security.ProtectPdfStage.publishing,
          security.ProtectPdfStage.completed,
        ]),
      );
      expect(firstProtect.last.status, security.PdfSecurityStatus.complete);
      expect(firstProtect.last.pageCount, 2);
      expect(
        File(firstProtect.last.outputPath!).uri.pathSegments.last,
        protectedName,
      );
      expect(
        File(secondProtect.last.outputPath!).uri.pathSegments.last,
        'รายงาน protected (1).pdf',
      );

      final protectedFile = File(firstProtect.last.outputPath!);
      final protectedBefore = await protectedFile.readAsBytes();
      final protectedInfo = await security.inspectUnlockPdfSource(
        sourcePath: protectedFile.path,
      );
      expect(
        protectedInfo.encryptionState,
        security.PdfEncryptionState.encryptedPasswordRequired,
      );
      expect(protectedInfo.defaultOutputName, 'รายงาน protected-unlocked.pdf');

      final wrongPassword = await security
          .unlockPdf(
            request: security.UnlockPdfRequest(
              sourcePath: protectedFile.path,
              destinationDirectory: unlockedDestination.path,
              outputName: 'รายงาน unlocked.pdf',
              password: 'not-the-password',
            ),
          )
          .toList();
      expect(wrongPassword.last.status, security.PdfSecurityStatus.failed);
      expect(
        wrongPassword.last.error?.code,
        rust_error.ApplicationErrorCode.incorrectPassword,
      );
      expect(await unlockedDestination.list().isEmpty, isTrue);

      final unlocked = await security
          .unlockPdf(
            request: security.UnlockPdfRequest(
              sourcePath: protectedFile.path,
              destinationDirectory: unlockedDestination.path,
              outputName: 'รายงาน unlocked.pdf',
              password: password,
            ),
          )
          .toList();
      expect(unlocked.last.status, security.PdfSecurityStatus.complete);
      expect(unlocked.last.pageCount, 2);
      expect(
        (await preview.openPdfDocument(sourcePath: unlocked.last.outputPath!))
            .pageCount,
        2,
      );
      expect(
        (await security.inspectProtectPdfSource(
          sourcePath: unlocked.last.outputPath!,
        )).encryptionState,
        security.PdfEncryptionState.unencrypted,
      );
      expect(await source.readAsBytes(), sourceBefore);
      expect(await protectedFile.readAsBytes(), protectedBefore);
    },
  );
}

File _fixture(String name) =>
    File('../../crates/ilikepdf_pdf/tests/fixtures/$name').absolute;
