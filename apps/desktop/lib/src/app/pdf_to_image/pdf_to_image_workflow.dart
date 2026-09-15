import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_application;
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
    this.inspectionProblem,
  });

  final String displayName;
  final String sourcePath;
  final String sourceDirectory;
  final int pageCount;
  final double? firstPageWidthPoints;
  final double? firstPageHeightPoints;
  final PdfExportProblem? inspectionProblem;
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

enum PdfImageFormat { png, jpg }

enum PdfDestinationMode { nextToSourceFiles, customFolder }

enum PdfBatchExportStatus { running, complete, completeWithErrors, failed }

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
  unsupportedImageFormat,
  malformedImage,
  imageDecodeFailed,
  imageOrientationFailed,
  duplicateOutputName,
  pdfDocumentCreationFailed,
  pdfPageCreationFailed,
  imagePlacementFailed,
  pdfSaveFailed,
  internal,
}

class PdfExportProblem {
  const PdfExportProblem({required this.code, required this.message});

  final PdfExportProblemCode code;
  final String message;
}

class PdfSelectionException implements Exception {
  const PdfSelectionException(this.problem);

  final PdfExportProblem problem;
}

class PdfBatchDocumentOutcome {
  const PdfBatchDocumentOutcome({
    required this.sourcePath,
    required this.displayName,
    required this.totalPageCount,
    required this.completedPageCount,
    required this.outputFiles,
    required this.error,
  });

  final String sourcePath;
  final String displayName;
  final int totalPageCount;
  final int completedPageCount;
  final List<String> outputFiles;
  final PdfExportProblem? error;
}

class PdfBatchExportUpdate {
  const PdfBatchExportUpdate({
    required this.status,
    required this.totalDocumentCount,
    required this.completedDocumentCount,
    required this.succeededDocumentCount,
    required this.failedDocumentCount,
    required this.currentDocumentIndex,
    required this.currentDocumentFilename,
    required this.totalPageCount,
    required this.completedPageCount,
    required this.currentPage,
    required this.documents,
    required this.error,
  });

  final PdfBatchExportStatus status;
  final int totalDocumentCount;
  final int completedDocumentCount;
  final int succeededDocumentCount;
  final int failedDocumentCount;
  final int? currentDocumentIndex;
  final String? currentDocumentFilename;
  final int totalPageCount;
  final int completedPageCount;
  final int? currentPage;
  final List<PdfBatchDocumentOutcome> documents;
  final PdfExportProblem? error;
}

abstract interface class PdfToImageWorkflow {
  Future<List<SelectedPdf>> selectPdfs();

  Future<List<SelectedPdf>> preparePdfPaths(List<String> sourcePaths);

  Future<String?> chooseDestinationDirectory();

  Stream<PdfBatchExportUpdate> exportBatch({
    required List<String> sourcePaths,
    required PdfDestinationMode destinationMode,
    required String? customDestinationDirectory,
    required PdfImageQuality quality,
    required PdfImageFormat format,
  });

  Future<RenderedPdfPage> renderFirstPage(String sourcePath);
}

class LocalPdfToImageWorkflow implements PdfToImageWorkflow {
  const LocalPdfToImageWorkflow();

  @override
  Future<List<SelectedPdf>> selectPdfs() async {
    const typeGroup = XTypeGroup(label: 'PDF documents', extensions: ['pdf']);
    final selected = await openFiles(acceptedTypeGroups: const [typeGroup]);
    return preparePdfPaths(
      selected.map((file) => file.path).toList(growable: false),
    );
  }

  @override
  Future<List<SelectedPdf>> preparePdfPaths(List<String> sourcePaths) async {
    final selected = <SelectedPdf>[];
    for (final sourcePath in sourcePaths) {
      final file = File(sourcePath);
      final filename = file.uri.pathSegments.isEmpty
          ? sourcePath
          : file.uri.pathSegments.last;
      final extensionIndex = filename.lastIndexOf('.');
      final extension = extensionIndex < 0
          ? ''
          : filename.substring(extensionIndex + 1).toLowerCase();
      if (extension != 'pdf') {
        throw const PdfSelectionException(
          PdfExportProblem(
            code: PdfExportProblemCode.invalidRequest,
            message: 'Only PDF documents are supported.',
          ),
        );
      }

      try {
        final info = await rust_preview.openPdfDocument(sourcePath: sourcePath);
        selected.add(
          SelectedPdf(
            displayName: filename,
            sourcePath: sourcePath,
            sourceDirectory: file.parent.path,
            pageCount: info.pageCount,
            firstPageWidthPoints: info.firstPageSize?.widthPoints,
            firstPageHeightPoints: info.firstPageSize?.heightPoints,
          ),
        );
      } on rust_application.ApplicationError catch (error) {
        selected.add(
          SelectedPdf(
            displayName: filename,
            sourcePath: sourcePath,
            sourceDirectory: file.parent.path,
            pageCount: 0,
            firstPageWidthPoints: null,
            firstPageHeightPoints: null,
            inspectionProblem: _mapError(error),
          ),
        );
      } on Object {
        selected.add(
          SelectedPdf(
            displayName: filename,
            sourcePath: sourcePath,
            sourceDirectory: file.parent.path,
            pageCount: 0,
            firstPageWidthPoints: null,
            firstPageHeightPoints: null,
            inspectionProblem: const PdfExportProblem(
              code: PdfExportProblemCode.internal,
              message: 'This PDF could not be opened.',
            ),
          ),
        );
      }
    }
    return selected;
  }

  @override
  Future<String?> chooseDestinationDirectory() =>
      getDirectoryPath(confirmButtonText: 'Choose export folder');

  @override
  Stream<PdfBatchExportUpdate> exportBatch({
    required List<String> sourcePaths,
    required PdfDestinationMode destinationMode,
    required String? customDestinationDirectory,
    required PdfImageQuality quality,
    required PdfImageFormat format,
  }) async* {
    final updates = rust_export.exportPdfBatchToImages(
      request: rust_export.ExportPdfBatchRequest(
        sourcePaths: sourcePaths,
        destinationMode: switch (destinationMode) {
          PdfDestinationMode.nextToSourceFiles =>
            rust_export.PdfBatchDestinationMode.nextToSourceFiles,
          PdfDestinationMode.customFolder =>
            rust_export.PdfBatchDestinationMode.customFolder,
        },
        customDestinationDirectory: customDestinationDirectory,
        quality: switch (quality) {
          PdfImageQuality.standard => rust_export.PdfExportQuality.standard,
          PdfImageQuality.highQuality =>
            rust_export.PdfExportQuality.highQuality,
        },
        format: switch (format) {
          PdfImageFormat.png => rust_export.PdfExportFormat.png,
          PdfImageFormat.jpg => rust_export.PdfExportFormat.jpg,
        },
      ),
    );

    await for (final update in updates) {
      yield PdfBatchExportUpdate(
        status: switch (update.status) {
          rust_export.PdfBatchExportStatus.running =>
            PdfBatchExportStatus.running,
          rust_export.PdfBatchExportStatus.complete =>
            PdfBatchExportStatus.complete,
          rust_export.PdfBatchExportStatus.completeWithErrors =>
            PdfBatchExportStatus.completeWithErrors,
          rust_export.PdfBatchExportStatus.failed =>
            PdfBatchExportStatus.failed,
        },
        totalDocumentCount: update.totalDocumentCount,
        completedDocumentCount: update.completedDocumentCount,
        succeededDocumentCount: update.succeededDocumentCount,
        failedDocumentCount: update.failedDocumentCount,
        currentDocumentIndex: update.currentDocumentIndex,
        currentDocumentFilename: update.currentDocumentFilename,
        totalPageCount: update.totalPageCount,
        completedPageCount: update.completedPageCount,
        currentPage: update.currentPage,
        documents: List.unmodifiable(
          update.documents.map(
            (document) => PdfBatchDocumentOutcome(
              sourcePath: document.sourcePath,
              displayName: document.displayName,
              totalPageCount: document.totalPageCount,
              completedPageCount: document.completedPageCount,
              outputFiles: List.unmodifiable(document.outputFiles),
              error: _mapError(document.error),
            ),
          ),
        ),
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
      rust_application.ApplicationErrorCode.structuralPdfRuntimeUnavailable =>
        PdfExportProblemCode.internal,
      rust_application.ApplicationErrorCode.structuralPdfRuntimeIncompatible =>
        PdfExportProblemCode.internal,
      rust_application.ApplicationErrorCode.structuralPdfLaunchFailed =>
        PdfExportProblemCode.internal,
      rust_application.ApplicationErrorCode.passwordRequired =>
        PdfExportProblemCode.internal,
      rust_application.ApplicationErrorCode.structuralPdfOperationFailed =>
        PdfExportProblemCode.internal,
      rust_application
          .ApplicationErrorCode
          .structuralPdfOutputValidationFailed =>
        PdfExportProblemCode.internal,
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
      rust_application.ApplicationErrorCode.unsupportedImageFormat =>
        PdfExportProblemCode.unsupportedImageFormat,
      rust_application.ApplicationErrorCode.malformedImage =>
        PdfExportProblemCode.malformedImage,
      rust_application.ApplicationErrorCode.imageDecodeFailed =>
        PdfExportProblemCode.imageDecodeFailed,
      rust_application.ApplicationErrorCode.imageOrientationFailed =>
        PdfExportProblemCode.imageOrientationFailed,
      rust_application.ApplicationErrorCode.duplicateOutputName =>
        PdfExportProblemCode.duplicateOutputName,
      rust_application.ApplicationErrorCode.pdfDocumentCreationFailed =>
        PdfExportProblemCode.pdfDocumentCreationFailed,
      rust_application.ApplicationErrorCode.pdfPageCreationFailed =>
        PdfExportProblemCode.pdfPageCreationFailed,
      rust_application.ApplicationErrorCode.imagePlacementFailed =>
        PdfExportProblemCode.imagePlacementFailed,
      rust_application.ApplicationErrorCode.pdfSaveFailed =>
        PdfExportProblemCode.pdfSaveFailed,
      rust_application.ApplicationErrorCode.internal =>
        PdfExportProblemCode.internal,
      _ => PdfExportProblemCode.internal,
    },
    message: error.message,
  );
}
