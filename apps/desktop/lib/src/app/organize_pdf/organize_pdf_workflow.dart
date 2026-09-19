import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_application;
import 'package:ilikepdf/src/rust/api/organize_pdf.dart' as rust_organize;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust_preview;

class SelectedOrganizePdf {
  const SelectedOrganizePdf({
    required this.displayName,
    required this.sourcePath,
    required this.sourceDirectory,
    required this.pageCount,
    required this.hasWarnings,
  });

  final String displayName;
  final String sourcePath;
  final String sourceDirectory;
  final int pageCount;
  final bool hasWarnings;
}

class RenderedOrganizePdfPage {
  const RenderedOrganizePdfPage({
    required this.outputPath,
    required this.widthPixels,
    required this.heightPixels,
  });

  final String outputPath;
  final int widthPixels;
  final int heightPixels;
}

enum OrganizePageRotation { none, clockwise90, halfTurn, counterClockwise90 }

enum OrganizePdfProgressStage {
  preparing,
  organizing,
  validating,
  publishing,
  completed,
}

enum OrganizePdfUpdateStatus { running, complete, failed }

class OrganizePdfProblem {
  const OrganizePdfProblem({required this.code, required this.message});

  final rust_application.ApplicationErrorCode code;
  final String message;
}

class OrganizePdfSelectionException implements Exception {
  const OrganizePdfSelectionException(this.problem);

  final OrganizePdfProblem problem;
}

class OrganizePdfSourceInput {
  const OrganizePdfSourceInput({
    required this.sourceId,
    required this.sourcePath,
    required this.pageCount,
    required this.hasWarnings,
  });

  final int sourceId;
  final String sourcePath;
  final int pageCount;
  final bool hasWarnings;
}

class OrganizePdfPageInput {
  const OrganizePdfPageInput({
    required this.pageItemId,
    required this.sourceId,
    required this.sourcePageIndex,
    required this.rotation,
  });

  final int pageItemId;
  final int sourceId;
  final int sourcePageIndex;
  final OrganizePageRotation rotation;
}

class OrganizePdfUpdate {
  const OrganizePdfUpdate({
    required this.status,
    required this.stage,
    required this.sourceCount,
    required this.pageCount,
    required this.outputPath,
    required this.warningSourceCount,
    required this.hasWarnings,
    required this.failedSourceId,
    required this.failedSourcePath,
    required this.failedPageItemId,
    required this.error,
  });

  final OrganizePdfUpdateStatus status;
  final OrganizePdfProgressStage stage;
  final int sourceCount;
  final int pageCount;
  final String? outputPath;
  final int warningSourceCount;
  final bool hasWarnings;
  final int? failedSourceId;
  final String? failedSourcePath;
  final int? failedPageItemId;
  final OrganizePdfProblem? error;
}

abstract interface class OrganizePdfWorkflow {
  Future<List<SelectedOrganizePdf>> selectPdfs({
    required List<String> existingSourcePaths,
  });

  Future<List<SelectedOrganizePdf>> preparePdfPaths({
    required List<String> existingSourcePaths,
    required List<String> candidateSourcePaths,
  });

  Future<String?> chooseDestinationDirectory();

  Future<RenderedOrganizePdfPage> renderPage({
    required String sourcePath,
    required int pageIndex,
  });

  Stream<OrganizePdfUpdate> organize({
    required List<OrganizePdfSourceInput> sources,
    required List<OrganizePdfPageInput> pageItems,
    required String destinationDirectory,
    required String outputName,
  });
}

class LocalOrganizePdfWorkflow implements OrganizePdfWorkflow {
  const LocalOrganizePdfWorkflow();

  @override
  Future<List<SelectedOrganizePdf>> selectPdfs({
    required List<String> existingSourcePaths,
  }) async {
    const typeGroup = XTypeGroup(label: 'PDF documents', extensions: ['pdf']);
    final selected = await openFiles(acceptedTypeGroups: const [typeGroup]);
    return preparePdfPaths(
      existingSourcePaths: existingSourcePaths,
      candidateSourcePaths: selected
          .map((file) => file.path)
          .toList(growable: false),
    );
  }

  @override
  Future<List<SelectedOrganizePdf>> preparePdfPaths({
    required List<String> existingSourcePaths,
    required List<String> candidateSourcePaths,
  }) async {
    if (candidateSourcePaths.isEmpty) return const [];
    if (candidateSourcePaths.any(
      (path) => !_filename(path).toLowerCase().endsWith('.pdf'),
    )) {
      throw const OrganizePdfSelectionException(
        OrganizePdfProblem(
          code: rust_application.ApplicationErrorCode.invalidRequest,
          message: 'Only PDF documents are supported.',
        ),
      );
    }
    try {
      final inspected = await rust_organize.inspectOrganizePdfSources(
        existingSourcePaths: existingSourcePaths,
        candidateSourcePaths: candidateSourcePaths,
      );
      return inspected
          .map(
            (source) => SelectedOrganizePdf(
              displayName: _filename(source.sourcePath),
              sourcePath: source.sourcePath,
              sourceDirectory: File(source.sourcePath).parent.path,
              pageCount: source.pageCount,
              hasWarnings: source.hasWarnings,
            ),
          )
          .toList(growable: false);
    } on rust_application.ApplicationError catch (error) {
      throw OrganizePdfSelectionException(
        OrganizePdfProblem(code: error.code, message: error.message),
      );
    }
  }

  @override
  Future<String?> chooseDestinationDirectory() =>
      getDirectoryPath(confirmButtonText: 'Choose output folder');

  @override
  Future<RenderedOrganizePdfPage> renderPage({
    required String sourcePath,
    required int pageIndex,
  }) async {
    final rendered = await rust_preview.renderPdfPage(
      request: rust_preview.RenderPdfPageRequest(
        sourcePath: sourcePath,
        pageIndex: pageIndex,
        targetWidth: 360,
        destinationPath: null,
      ),
    );
    return RenderedOrganizePdfPage(
      outputPath: rendered.outputPath,
      widthPixels: rendered.widthPixels,
      heightPixels: rendered.heightPixels,
    );
  }

  @override
  Stream<OrganizePdfUpdate> organize({
    required List<OrganizePdfSourceInput> sources,
    required List<OrganizePdfPageInput> pageItems,
    required String destinationDirectory,
    required String outputName,
  }) async* {
    final updates = rust_organize.organizePdf(
      request: rust_organize.OrganizePdfRequest(
        sources: sources
            .map(
              (source) => rust_organize.OrganizePdfSource(
                sourceId: source.sourceId,
                sourcePath: source.sourcePath,
                pageCount: source.pageCount,
                hasWarnings: source.hasWarnings,
              ),
            )
            .toList(growable: false),
        pageItems: pageItems
            .map(
              (page) => rust_organize.OrganizePdfPageItem(
                pageItemId: page.pageItemId,
                sourceId: page.sourceId,
                sourcePageIndex: page.sourcePageIndex,
                rotation: switch (page.rotation) {
                  OrganizePageRotation.none =>
                    rust_organize.OrganizePdfPageRotation.none,
                  OrganizePageRotation.clockwise90 =>
                    rust_organize.OrganizePdfPageRotation.clockwise90,
                  OrganizePageRotation.halfTurn =>
                    rust_organize.OrganizePdfPageRotation.halfTurn,
                  OrganizePageRotation.counterClockwise90 =>
                    rust_organize.OrganizePdfPageRotation.counterClockwise90,
                },
              ),
            )
            .toList(growable: false),
        destinationDirectory: destinationDirectory,
        outputName: outputName,
      ),
    );
    await for (final update in updates) {
      yield OrganizePdfUpdate(
        status: switch (update.status) {
          rust_organize.OrganizePdfStatus.running =>
            OrganizePdfUpdateStatus.running,
          rust_organize.OrganizePdfStatus.complete =>
            OrganizePdfUpdateStatus.complete,
          rust_organize.OrganizePdfStatus.failed =>
            OrganizePdfUpdateStatus.failed,
        },
        stage: switch (update.stage) {
          rust_organize.OrganizePdfStage.preparing =>
            OrganizePdfProgressStage.preparing,
          rust_organize.OrganizePdfStage.organizing =>
            OrganizePdfProgressStage.organizing,
          rust_organize.OrganizePdfStage.validating =>
            OrganizePdfProgressStage.validating,
          rust_organize.OrganizePdfStage.publishing =>
            OrganizePdfProgressStage.publishing,
          rust_organize.OrganizePdfStage.completed =>
            OrganizePdfProgressStage.completed,
        },
        sourceCount: update.sourceCount,
        pageCount: update.pageCount,
        outputPath: update.outputPath,
        warningSourceCount: update.warningSourceCount,
        hasWarnings: update.hasWarnings,
        failedSourceId: update.failedSourceId,
        failedSourcePath: update.failedSourcePath,
        failedPageItemId: update.failedPageItemId,
        error: update.error == null
            ? null
            : OrganizePdfProblem(
                code: update.error!.code,
                message: update.error!.message,
              ),
      );
    }
  }
}

String? normalizeOrganizeOutputName(String value) {
  final name = value.trim();
  if (name.isEmpty ||
      name == '.' ||
      name == '..' ||
      name.endsWith(' ') ||
      name.endsWith('.') ||
      RegExp(r'[\x00-\x1f<>:"/\\|?*]').hasMatch(name)) {
    return null;
  }
  final normalized = name.toLowerCase().endsWith('.pdf') ? name : '$name.pdf';
  final stem = normalized.substring(0, normalized.length - 4);
  final basename = stem.split('.').first.toUpperCase();
  const reserved = {
    'CON',
    'PRN',
    'AUX',
    'NUL',
    'COM1',
    'COM2',
    'COM3',
    'COM4',
    'COM5',
    'COM6',
    'COM7',
    'COM8',
    'COM9',
    'LPT1',
    'LPT2',
    'LPT3',
    'LPT4',
    'LPT5',
    'LPT6',
    'LPT7',
    'LPT8',
    'LPT9',
  };
  return stem.isEmpty || reserved.contains(basename) ? null : normalized;
}

String _filename(String path) {
  final segments = File(path).uri.pathSegments;
  return segments.isEmpty ? path : segments.last;
}
