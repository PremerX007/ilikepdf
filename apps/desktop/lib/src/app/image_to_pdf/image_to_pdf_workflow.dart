import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/rust/api/application.dart' as rust_application;
import 'package:ilikepdf/src/rust/api/image_to_pdf.dart' as rust_image_pdf;

class SelectedImage {
  const SelectedImage({
    required this.displayName,
    required this.sourcePath,
    required this.sourceDirectory,
  });

  final String displayName;
  final String sourcePath;
  final String sourceDirectory;
}

enum ImagePageSize { fit, a4, usLetter }

enum ImagePageOrientation { portrait, landscape }

enum ImagePageMargin { none, small, big }

enum ImagePdfCreationStatus { running, complete, failed }

enum ImagePdfProblemCode {
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

class ImagePdfProblem {
  const ImagePdfProblem({required this.code, required this.message});

  final ImagePdfProblemCode code;
  final String message;
}

class ImageSelectionException implements Exception {
  const ImageSelectionException(this.problem);

  final ImagePdfProblem problem;
}

class ImagePdfCreationUpdate {
  const ImagePdfCreationUpdate({
    required this.status,
    required this.totalImageCount,
    required this.completedImageCount,
    required this.currentImage,
    required this.outputFiles,
    required this.error,
  });

  final ImagePdfCreationStatus status;
  final int totalImageCount;
  final int completedImageCount;
  final int? currentImage;
  final List<String> outputFiles;
  final ImagePdfProblem? error;
}

abstract interface class ImageToPdfWorkflow {
  Future<List<SelectedImage>> selectImages();

  Future<List<SelectedImage>> prepareImagePaths(List<String> sourcePaths);

  Future<String?> chooseDestinationDirectory();

  Stream<ImagePdfCreationUpdate> createPdfs({
    required List<String> sourcePaths,
    required String destinationDirectory,
    required ImagePageSize pageSize,
    required ImagePageOrientation orientation,
    required ImagePageMargin margin,
    required bool merge,
  });
}

class LocalImageToPdfWorkflow implements ImageToPdfWorkflow {
  const LocalImageToPdfWorkflow();

  @override
  Future<List<SelectedImage>> selectImages() async {
    const typeGroup = XTypeGroup(
      label: 'Supported images',
      extensions: ['jpg', 'jpeg', 'png', 'webp'],
    );
    final selected = await openFiles(acceptedTypeGroups: const [typeGroup]);
    return prepareImagePaths(
      selected.map((file) => file.path).toList(growable: false),
    );
  }

  @override
  Future<List<SelectedImage>> prepareImagePaths(
    List<String> sourcePaths,
  ) async {
    const supportedExtensions = {'jpg', 'jpeg', 'png', 'webp'};
    final images = <SelectedImage>[];
    for (final sourcePath in sourcePaths) {
      final file = File(sourcePath);
      final filename = file.uri.pathSegments.isEmpty
          ? sourcePath
          : file.uri.pathSegments.last;
      final separator = filename.lastIndexOf('.');
      final extension = separator < 0
          ? ''
          : filename.substring(separator + 1).toLowerCase();
      if (!supportedExtensions.contains(extension)) {
        throw const ImageSelectionException(
          ImagePdfProblem(
            code: ImagePdfProblemCode.unsupportedImageFormat,
            message: 'Only JPG, JPEG, PNG, and WebP images are supported.',
          ),
        );
      }
      images.add(
        SelectedImage(
          displayName: filename,
          sourcePath: sourcePath,
          sourceDirectory: file.parent.path,
        ),
      );
    }
    return images;
  }

  @override
  Future<String?> chooseDestinationDirectory() =>
      getDirectoryPath(confirmButtonText: 'Choose output folder');

  @override
  Stream<ImagePdfCreationUpdate> createPdfs({
    required List<String> sourcePaths,
    required String destinationDirectory,
    required ImagePageSize pageSize,
    required ImagePageOrientation orientation,
    required ImagePageMargin margin,
    required bool merge,
  }) async* {
    final updates = rust_image_pdf.createPdfsFromImages(
      request: rust_image_pdf.CreateImagePdfRequest(
        sourcePaths: sourcePaths,
        destinationDirectory: destinationDirectory,
        pageSize: switch (pageSize) {
          ImagePageSize.fit => rust_image_pdf.ImagePdfPageSize.fit,
          ImagePageSize.a4 => rust_image_pdf.ImagePdfPageSize.a4,
          ImagePageSize.usLetter => rust_image_pdf.ImagePdfPageSize.usLetter,
        },
        orientation: switch (orientation) {
          ImagePageOrientation.portrait =>
            rust_image_pdf.ImagePdfOrientation.portrait,
          ImagePageOrientation.landscape =>
            rust_image_pdf.ImagePdfOrientation.landscape,
        },
        margin: switch (margin) {
          ImagePageMargin.none => rust_image_pdf.ImagePdfMargin.none,
          ImagePageMargin.small => rust_image_pdf.ImagePdfMargin.small,
          ImagePageMargin.big => rust_image_pdf.ImagePdfMargin.big,
        },
        merge: merge,
      ),
    );

    await for (final update in updates) {
      yield ImagePdfCreationUpdate(
        status: switch (update.status) {
          rust_image_pdf.ImagePdfStatus.running =>
            ImagePdfCreationStatus.running,
          rust_image_pdf.ImagePdfStatus.complete =>
            ImagePdfCreationStatus.complete,
          rust_image_pdf.ImagePdfStatus.failed => ImagePdfCreationStatus.failed,
        },
        totalImageCount: update.totalImageCount,
        completedImageCount: update.completedImageCount,
        currentImage: update.currentImage,
        outputFiles: List.unmodifiable(update.outputFiles),
        error: _mapError(update.error),
      );
    }
  }

  ImagePdfProblem? _mapError(rust_application.ApplicationError? error) {
    if (error == null) {
      return null;
    }
    return ImagePdfProblem(
      code: switch (error.code) {
        rust_application.ApplicationErrorCode.sourceNotFound =>
          ImagePdfProblemCode.sourceNotFound,
        rust_application.ApplicationErrorCode.sourceNotFile =>
          ImagePdfProblemCode.sourceNotFile,
        rust_application.ApplicationErrorCode.sourceUnreadable =>
          ImagePdfProblemCode.sourceUnreadable,
        rust_application.ApplicationErrorCode.invalidPdf =>
          ImagePdfProblemCode.invalidPdf,
        rust_application.ApplicationErrorCode.pageOutOfBounds =>
          ImagePdfProblemCode.pageOutOfBounds,
        rust_application.ApplicationErrorCode.invalidRequest =>
          ImagePdfProblemCode.invalidRequest,
        rust_application.ApplicationErrorCode.pdfRuntimeUnavailable =>
          ImagePdfProblemCode.pdfRuntimeUnavailable,
        rust_application.ApplicationErrorCode.invalidOutputDirectory =>
          ImagePdfProblemCode.invalidOutputDirectory,
        rust_application.ApplicationErrorCode.permissionDenied =>
          ImagePdfProblemCode.permissionDenied,
        rust_application.ApplicationErrorCode.outputNotWritable =>
          ImagePdfProblemCode.outputNotWritable,
        rust_application.ApplicationErrorCode.outputAlreadyExists =>
          ImagePdfProblemCode.outputAlreadyExists,
        rust_application.ApplicationErrorCode.outputWriteFailed =>
          ImagePdfProblemCode.outputWriteFailed,
        rust_application.ApplicationErrorCode.renderingFailed =>
          ImagePdfProblemCode.renderingFailed,
        rust_application.ApplicationErrorCode.encodingFailed =>
          ImagePdfProblemCode.encodingFailed,
        rust_application.ApplicationErrorCode.unsupportedImageFormat =>
          ImagePdfProblemCode.unsupportedImageFormat,
        rust_application.ApplicationErrorCode.malformedImage =>
          ImagePdfProblemCode.malformedImage,
        rust_application.ApplicationErrorCode.imageDecodeFailed =>
          ImagePdfProblemCode.imageDecodeFailed,
        rust_application.ApplicationErrorCode.imageOrientationFailed =>
          ImagePdfProblemCode.imageOrientationFailed,
        rust_application.ApplicationErrorCode.duplicateOutputName =>
          ImagePdfProblemCode.duplicateOutputName,
        rust_application.ApplicationErrorCode.pdfDocumentCreationFailed =>
          ImagePdfProblemCode.pdfDocumentCreationFailed,
        rust_application.ApplicationErrorCode.pdfPageCreationFailed =>
          ImagePdfProblemCode.pdfPageCreationFailed,
        rust_application.ApplicationErrorCode.imagePlacementFailed =>
          ImagePdfProblemCode.imagePlacementFailed,
        rust_application.ApplicationErrorCode.pdfSaveFailed =>
          ImagePdfProblemCode.pdfSaveFailed,
        rust_application.ApplicationErrorCode.internal =>
          ImagePdfProblemCode.internal,
      },
      message: error.message,
    );
  }
}
