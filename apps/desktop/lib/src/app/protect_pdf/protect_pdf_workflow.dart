import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/app/shared/pdf_encryption_state.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_error;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust_preview;
import 'package:ilikepdf/src/rust/api/pdf_security.dart' as rust_security;

class SelectedProtectPdf {
  const SelectedProtectPdf({
    required this.displayName,
    required this.sourcePath,
    required this.sourceDirectory,
    required this.pageCount,
    required this.sizeBytes,
    required this.encryptionState,
    required this.hasWarnings,
    required this.defaultOutputName,
  });

  final String displayName;
  final String sourcePath;
  final String sourceDirectory;
  final int? pageCount;
  final int sizeBytes;
  final PdfEncryptionState encryptionState;
  final bool hasWarnings;
  final String defaultOutputName;
}

class RenderedProtectPdfPage {
  const RenderedProtectPdfPage({required this.outputPath});

  final String outputPath;
}

enum ProtectPdfProgressStage {
  preparing,
  protecting,
  validating,
  publishing,
  completed,
}

enum ProtectPdfUpdateStatus { running, complete, failed }

class ProtectPdfProblem {
  const ProtectPdfProblem({required this.code, required this.message});

  final rust_error.ApplicationErrorCode code;
  final String message;
}

class ProtectPdfSelectionException implements Exception {
  const ProtectPdfSelectionException(this.problem);

  final ProtectPdfProblem problem;
}

class ProtectPdfUpdate {
  const ProtectPdfUpdate({
    required this.status,
    required this.stage,
    required this.pageCount,
    required this.outputPath,
    required this.hasWarnings,
    required this.error,
  });

  final ProtectPdfUpdateStatus status;
  final ProtectPdfProgressStage stage;
  final int pageCount;
  final String? outputPath;
  final bool hasWarnings;
  final ProtectPdfProblem? error;
}

abstract interface class ProtectPdfWorkflow {
  Future<SelectedProtectPdf?> selectPdf();

  Future<SelectedProtectPdf> preparePdfPaths(List<String> sourcePaths);

  Future<String?> chooseDestinationDirectory();

  Future<RenderedProtectPdfPage> renderFirstPage(String sourcePath);

  Stream<ProtectPdfUpdate> protect({
    required String sourcePath,
    required String destinationDirectory,
    required String outputName,
    required String password,
    required String confirmation,
  });
}

class LocalProtectPdfWorkflow implements ProtectPdfWorkflow {
  const LocalProtectPdfWorkflow();

  @override
  Future<SelectedProtectPdf?> selectPdf() async {
    const typeGroup = XTypeGroup(label: 'PDF document', extensions: ['pdf']);
    final selected = await openFile(acceptedTypeGroups: const [typeGroup]);
    return selected == null ? null : preparePdfPaths([selected.path]);
  }

  @override
  Future<SelectedProtectPdf> preparePdfPaths(List<String> sourcePaths) async {
    if (sourcePaths.length != 1) {
      throw const ProtectPdfSelectionException(
        ProtectPdfProblem(
          code: rust_error.ApplicationErrorCode.invalidRequest,
          message: 'Protect PDF supports one PDF at a time.',
        ),
      );
    }
    final sourcePath = sourcePaths.single;
    final file = File(sourcePath);
    final name = file.uri.pathSegments.isEmpty
        ? sourcePath
        : file.uri.pathSegments.last;
    if (!name.toLowerCase().endsWith('.pdf')) {
      throw const ProtectPdfSelectionException(
        ProtectPdfProblem(
          code: rust_error.ApplicationErrorCode.invalidRequest,
          message: 'Select one PDF document.',
        ),
      );
    }
    try {
      final info = await rust_security.inspectProtectPdfSource(
        sourcePath: sourcePath,
      );
      return SelectedProtectPdf(
        displayName: name,
        sourcePath: info.sourcePath,
        sourceDirectory: File(info.sourcePath).parent.path,
        pageCount: info.pageCount,
        sizeBytes: info.sizeBytes.toInt(),
        encryptionState: _encryptionState(info.encryptionState),
        hasWarnings: info.hasWarnings,
        defaultOutputName: info.defaultOutputName,
      );
    } on rust_error.ApplicationError catch (error) {
      throw ProtectPdfSelectionException(
        ProtectPdfProblem(code: error.code, message: error.message),
      );
    } on ProtectPdfSelectionException {
      rethrow;
    } on Object {
      throw const ProtectPdfSelectionException(
        ProtectPdfProblem(
          code: rust_error.ApplicationErrorCode.internal,
          message: 'This PDF could not be inspected.',
        ),
      );
    }
  }

  @override
  Future<String?> chooseDestinationDirectory() =>
      getDirectoryPath(confirmButtonText: 'Choose output folder');

  @override
  Future<RenderedProtectPdfPage> renderFirstPage(String sourcePath) async {
    final result = await rust_preview.renderPdfPage(
      request: rust_preview.RenderPdfPageRequest(
        sourcePath: sourcePath,
        pageIndex: 0,
        targetWidth: 1000,
        destinationPath: null,
      ),
    );
    return RenderedProtectPdfPage(outputPath: result.outputPath);
  }

  @override
  Stream<ProtectPdfUpdate> protect({
    required String sourcePath,
    required String destinationDirectory,
    required String outputName,
    required String password,
    required String confirmation,
  }) async* {
    final updates = rust_security.protectPdf(
      request: rust_security.ProtectPdfRequest(
        sourcePath: sourcePath,
        destinationDirectory: destinationDirectory,
        outputName: outputName,
        password: password,
        confirmation: confirmation,
      ),
    );
    await for (final update in updates) {
      yield ProtectPdfUpdate(
        status: switch (update.status) {
          rust_security.PdfSecurityStatus.running =>
            ProtectPdfUpdateStatus.running,
          rust_security.PdfSecurityStatus.complete =>
            ProtectPdfUpdateStatus.complete,
          rust_security.PdfSecurityStatus.failed =>
            ProtectPdfUpdateStatus.failed,
        },
        stage: switch (update.stage) {
          rust_security.ProtectPdfStage.preparing =>
            ProtectPdfProgressStage.preparing,
          rust_security.ProtectPdfStage.protecting =>
            ProtectPdfProgressStage.protecting,
          rust_security.ProtectPdfStage.validating =>
            ProtectPdfProgressStage.validating,
          rust_security.ProtectPdfStage.publishing =>
            ProtectPdfProgressStage.publishing,
          rust_security.ProtectPdfStage.completed =>
            ProtectPdfProgressStage.completed,
        },
        pageCount: update.pageCount,
        outputPath: update.outputPath,
        hasWarnings: update.hasWarnings,
        error: update.error == null
            ? null
            : ProtectPdfProblem(
                code: update.error!.code,
                message: update.error!.message,
              ),
      );
    }
  }
}

PdfEncryptionState _encryptionState(rust_security.PdfEncryptionState state) =>
    switch (state) {
      rust_security.PdfEncryptionState.unencrypted =>
        PdfEncryptionState.unencrypted,
      rust_security.PdfEncryptionState.encryptedNoPasswordRequired =>
        PdfEncryptionState.encryptedNoPasswordRequired,
      rust_security.PdfEncryptionState.encryptedPasswordRequired =>
        PdfEncryptionState.encryptedPasswordRequired,
    };
