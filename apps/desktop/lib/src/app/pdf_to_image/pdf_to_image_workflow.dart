import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/rust/api/application.dart' as rust_application;
import 'package:ilikepdf/src/rust/api/pdf_export.dart' as rust_export;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust_preview;

class SelectedPdf {
  const SelectedPdf({
    required this.displayName,
    required this.sourcePath,
    required this.sourceDirectory,
    required this.pageCount,
    required this.firstPageWidthPoints,
    required this.firstPageHeightPoints,
  });

  final String displayName;
  final String sourcePath;
  final String sourceDirectory;
  final int pageCount;
  final double? firstPageWidthPoints;
  final double? firstPageHeightPoints;
}

class RenderedPdfPage {
  const RenderedPdfPage({
    required this.outputPath,
    required this.widthPixels,
    required this.heightPixels,
  });

  final String outputPath;
  final int widthPixels;
  final int heightPixels;
}

enum PdfImageQuality {
  standard(150),
  highQuality(300);

  const PdfImageQuality(this.dpi);

  final int dpi;
}

enum PdfImageExportStatus { running, complete, failed }

enum PdfExportProblemCode {
  sourceNotFound,
  sourceNotFile,
  sourceUnreadable,
  invalidPdf,
  pageOutOfBounds,
  invalidRequest,
  pdfRuntimeUnavailable,
  invalidOutputDirectory,
  permissionDenied,
  outputNotWritable,
  outputAlreadyExists,
  outputWriteFailed,
  renderingFailed,
  encodingFailed,
  internal,
}

class PdfExportProblem {
  const PdfExportProblem({required this.code, required this.message});

  final PdfExportProblemCode code;
  final String message;
}

class PdfImageExportUpdate {
  const PdfImageExportUpdate({
    required this.status,
    required this.totalPageCount,
    required this.completedPageCount,
    required this.currentPage,
    required this.outputFiles,
    required this.error,
  });

  final PdfImageExportStatus status;
  final int totalPageCount;
  final int completedPageCount;
  final int? currentPage;
  final List<String> outputFiles;
  final PdfExportProblem? error;
}

abstract interface class PdfToImageWorkflow {
  Future<SelectedPdf?> selectAndInspect();

  Future<String?> chooseDestinationDirectory();

  Stream<PdfImageExportUpdate> exportAllPages({
    required String sourcePath,
    required String destinationDirectory,
    required PdfImageQuality quality,
  });

  Future<RenderedPdfPage> renderFirstPage(String sourcePath);
}

class LocalPdfToImageWorkflow implements PdfToImageWorkflow {
  const LocalPdfToImageWorkflow();

  @override
  Future<SelectedPdf?> selectAndInspect() async {
    const typeGroup = XTypeGroup(label: 'PDF documents', extensions: ['pdf']);
    final selected = await openFile(acceptedTypeGroups: const [typeGroup]);
    if (selected == null) {
      return null;
    }

    final info = await rust_preview.openPdfDocument(sourcePath: selected.path);
    return SelectedPdf(
      displayName: selected.name,
      sourcePath: selected.path,
      sourceDirectory: File(selected.path).parent.path,
      pageCount: info.pageCount,
      firstPageWidthPoints: info.firstPageSize?.widthPoints,
      firstPageHeightPoints: info.firstPageSize?.heightPoints,
    );
  }

  @override
  Future<String?> chooseDestinationDirectory() =>
      getDirectoryPath(confirmButtonText: 'Choose export folder');

  @override
  Stream<PdfImageExportUpdate> exportAllPages({
    required String sourcePath,
    required String destinationDirectory,
    required PdfImageQuality quality,
  }) async* {
    final updates = rust_export.exportPdfToImages(
      request: rust_export.ExportPdfToImagesRequest(
        sourcePath: sourcePath,
        destinationDirectory: destinationDirectory,
        quality: switch (quality) {
          PdfImageQuality.standard => rust_export.PdfExportQuality.standard,
          PdfImageQuality.highQuality =>
            rust_export.PdfExportQuality.highQuality,
        },
      ),
    );

    await for (final update in updates) {
      yield PdfImageExportUpdate(
        status: switch (update.status) {
          rust_export.PdfExportStatus.running => PdfImageExportStatus.running,
          rust_export.PdfExportStatus.complete => PdfImageExportStatus.complete,
          rust_export.PdfExportStatus.failed => PdfImageExportStatus.failed,
        },
        totalPageCount: update.totalPageCount,
        completedPageCount: update.completedPageCount,
        currentPage: update.currentPage,
        outputFiles: List.unmodifiable(update.outputFiles),
        error: _mapError(update.error),
      );
    }
  }

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) async {
    final result = await rust_preview.renderPdfPage(
      request: rust_preview.RenderPdfPageRequest(
        sourcePath: sourcePath,
        pageIndex: 0,
        targetWidth: 1000,
        destinationPath: null,
      ),
    );
    return RenderedPdfPage(
      outputPath: result.outputPath,
      widthPixels: result.widthPixels,
      heightPixels: result.heightPixels,
    );
  }

  PdfExportProblem? _mapError(rust_application.ApplicationError? error) {
    if (error == null) {
      return null;
    }

    return PdfExportProblem(
      code: switch (error.code) {
        rust_application.ApplicationErrorCode.sourceNotFound =>
          PdfExportProblemCode.sourceNotFound,
        rust_application.ApplicationErrorCode.sourceNotFile =>
          PdfExportProblemCode.sourceNotFile,
        rust_application.ApplicationErrorCode.sourceUnreadable =>
          PdfExportProblemCode.sourceUnreadable,
        rust_application.ApplicationErrorCode.invalidPdf =>
          PdfExportProblemCode.invalidPdf,
        rust_application.ApplicationErrorCode.pageOutOfBounds =>
          PdfExportProblemCode.pageOutOfBounds,
        rust_application.ApplicationErrorCode.invalidRequest =>
          PdfExportProblemCode.invalidRequest,
        rust_application.ApplicationErrorCode.pdfRuntimeUnavailable =>
          PdfExportProblemCode.pdfRuntimeUnavailable,
        rust_application.ApplicationErrorCode.invalidOutputDirectory =>
          PdfExportProblemCode.invalidOutputDirectory,
        rust_application.ApplicationErrorCode.permissionDenied =>
          PdfExportProblemCode.permissionDenied,
        rust_application.ApplicationErrorCode.outputNotWritable =>
          PdfExportProblemCode.outputNotWritable,
        rust_application.ApplicationErrorCode.outputAlreadyExists =>
          PdfExportProblemCode.outputAlreadyExists,
        rust_application.ApplicationErrorCode.outputWriteFailed =>
          PdfExportProblemCode.outputWriteFailed,
        rust_application.ApplicationErrorCode.renderingFailed =>
          PdfExportProblemCode.renderingFailed,
        rust_application.ApplicationErrorCode.encodingFailed =>
          PdfExportProblemCode.encodingFailed,
        rust_application.ApplicationErrorCode.internal =>
          PdfExportProblemCode.internal,
      },
      message: error.message,
    );
  }
}
