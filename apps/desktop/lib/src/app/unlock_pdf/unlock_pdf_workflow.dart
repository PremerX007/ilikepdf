import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/app/shared/pdf_encryption_state.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_error;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust_preview;
import 'package:ilikepdf/src/rust/api/pdf_security.dart' as rust_security;

class SelectedUnlockPdf {
  const SelectedUnlockPdf({
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

class RenderedUnlockPdfPage {
  const RenderedUnlockPdfPage({required this.outputPath});

  final String outputPath;
}

enum UnlockPdfProgressStage {
  preparing,
  unlocking,
  validating,
  publishing,
  completed,
}

enum UnlockPdfUpdateStatus { running, complete, failed }

class UnlockPdfProblem {
  const UnlockPdfProblem({required this.code, required this.message});

  final rust_error.ApplicationErrorCode code;
  final String message;
}

class UnlockPdfSelectionException implements Exception {
  const UnlockPdfSelectionException(this.problem);

  final UnlockPdfProblem problem;
}

class UnlockPdfUpdate {
  const UnlockPdfUpdate({
    required this.status,
    required this.stage,
    required this.pageCount,
    required this.outputPath,
    required this.hasWarnings,
    required this.error,
  });

  final UnlockPdfUpdateStatus status;
  final UnlockPdfProgressStage stage;
  final int pageCount;
  final String? outputPath;
  final bool hasWarnings;
  final UnlockPdfProblem? error;
}

abstract interface class UnlockPdfWorkflow {
  Future<SelectedUnlockPdf?> selectPdf();

  Future<SelectedUnlockPdf> preparePdfPaths(List<String> sourcePaths);

  Future<String?> chooseDestinationDirectory();

  Future<RenderedUnlockPdfPage> renderFirstPage(String sourcePath);

  Stream<UnlockPdfUpdate> unlock({
    required String sourcePath,
    required String destinationDirectory,
    required String outputName,
    required String? password,
  });
}

class LocalUnlockPdfWorkflow implements UnlockPdfWorkflow {
  const LocalUnlockPdfWorkflow();

  @override
  Future<SelectedUnlockPdf?> selectPdf() async {
    const typeGroup = XTypeGroup(label: 'PDF document', extensions: ['pdf']);
    final selected = await openFile(acceptedTypeGroups: const [typeGroup]);
    return selected == null ? null : preparePdfPaths([selected.path]);
  }

  @override
  Future<SelectedUnlockPdf> preparePdfPaths(List<String> sourcePaths) async {
    if (sourcePaths.length != 1) {
      throw const UnlockPdfSelectionException(
        UnlockPdfProblem(
          code: rust_error.ApplicationErrorCode.invalidRequest,
          message: 'Unlock PDF supports one PDF at a time.',
        ),
      );
    }
    final sourcePath = sourcePaths.single;
    final file = File(sourcePath);
    final name = file.uri.pathSegments.isEmpty
        ? sourcePath
        : file.uri.pathSegments.last;
    if (!name.toLowerCase().endsWith('.pdf')) {
      throw const UnlockPdfSelectionException(
        UnlockPdfProblem(
          code: rust_error.ApplicationErrorCode.invalidRequest,
          message: 'Select one PDF document.',
        ),
      );
    }
    try {
      final info = await rust_security.inspectUnlockPdfSource(
        sourcePath: sourcePath,
      );
      return SelectedUnlockPdf(
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
      throw UnlockPdfSelectionException(
        UnlockPdfProblem(code: error.code, message: error.message),
      );
    } on UnlockPdfSelectionException {
      rethrow;
    } on Object {
      throw const UnlockPdfSelectionException(
        UnlockPdfProblem(
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
  Future<RenderedUnlockPdfPage> renderFirstPage(String sourcePath) async {
    final result = await rust_preview.renderPdfPage(
      request: rust_preview.RenderPdfPageRequest(
        sourcePath: sourcePath,
        pageIndex: 0,
        targetWidth: 1000,
        destinationPath: null,
      ),
    );
    return RenderedUnlockPdfPage(outputPath: result.outputPath);
  }

  @override
  Stream<UnlockPdfUpdate> unlock({
    required String sourcePath,
    required String destinationDirectory,
    required String outputName,
    required String? password,
  }) async* {
    final updates = rust_security.unlockPdf(
      request: rust_security.UnlockPdfRequest(
        sourcePath: sourcePath,
        destinationDirectory: destinationDirectory,
        outputName: outputName,
        password: password,
      ),
    );
    await for (final update in updates) {
      yield UnlockPdfUpdate(
        status: switch (update.status) {
          rust_security.PdfSecurityStatus.running =>
            UnlockPdfUpdateStatus.running,
          rust_security.PdfSecurityStatus.complete =>
            UnlockPdfUpdateStatus.complete,
          rust_security.PdfSecurityStatus.failed =>
            UnlockPdfUpdateStatus.failed,
        },
        stage: switch (update.stage) {
          rust_security.UnlockPdfStage.preparing =>
            UnlockPdfProgressStage.preparing,
          rust_security.UnlockPdfStage.unlocking =>
            UnlockPdfProgressStage.unlocking,
          rust_security.UnlockPdfStage.validating =>
            UnlockPdfProgressStage.validating,
          rust_security.UnlockPdfStage.publishing =>
            UnlockPdfProgressStage.publishing,
          rust_security.UnlockPdfStage.completed =>
            UnlockPdfProgressStage.completed,
        },
        pageCount: update.pageCount,
        outputPath: update.outputPath,
        hasWarnings: update.hasWarnings,
        error: update.error == null
            ? null
            : UnlockPdfProblem(
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
