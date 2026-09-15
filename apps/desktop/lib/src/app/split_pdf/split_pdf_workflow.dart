import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_application;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust_preview;
import 'package:ilikepdf/src/rust/api/split_pdf.dart' as rust_split;

enum SplitPdfMode { everyPage, everyNPages, splitAfterPages, maximumFileSize }

class SelectedSplitPdf {
  const SelectedSplitPdf({
    required this.displayName,
    required this.sourcePath,
    required this.sourceDirectory,
    required this.pageCount,
    required this.sizeBytes,
    required this.hasWarnings,
  });

  final String displayName;
  final String sourcePath;
  final String sourceDirectory;
  final int pageCount;
  final int sizeBytes;
  final bool hasWarnings;
}

class RenderedSplitPdfPage {
  const RenderedSplitPdfPage({required this.outputPath});

  final String outputPath;
}

class SplitPdfRange {
  const SplitPdfRange({required this.firstPage, required this.lastPage});

  final int firstPage;
  final int lastPage;

  int get pageCount => lastPage - firstPage + 1;
}

class SplitPdfPart {
  const SplitPdfPart({
    required this.outputPath,
    required this.range,
    required this.sizeBytes,
  });

  final String outputPath;
  final SplitPdfRange range;
  final int sizeBytes;
}

enum SplitPdfProgressStage {
  preparing,
  findingSplitPoints,
  creating,
  validating,
  publishing,
  completed,
}

enum SplitPdfUpdateStatus { running, complete, failed }

class SplitPdfProblem {
  const SplitPdfProblem({required this.code, required this.message});

  final rust_application.ApplicationErrorCode code;
  final String message;
}

class SplitPdfSelectionException implements Exception {
  const SplitPdfSelectionException(this.problem);

  final SplitPdfProblem problem;
}

class SplitPdfUpdate {
  const SplitPdfUpdate({
    required this.status,
    required this.stage,
    required this.sourcePageCount,
    required this.currentPart,
    required this.totalParts,
    required this.outputDirectory,
    required this.parts,
    required this.hasWarnings,
    required this.failedPageNumber,
    required this.actualSizeBytes,
    required this.limitSizeBytes,
    required this.error,
  });

  final SplitPdfUpdateStatus status;
  final SplitPdfProgressStage stage;
  final int sourcePageCount;
  final int? currentPart;
  final int? totalParts;
  final String? outputDirectory;
  final List<SplitPdfPart> parts;
  final bool hasWarnings;
  final int? failedPageNumber;
  final int? actualSizeBytes;
  final int? limitSizeBytes;
  final SplitPdfProblem? error;
}

abstract interface class SplitPdfWorkflow {
  Future<SelectedSplitPdf?> selectPdf();

  Future<SelectedSplitPdf> preparePdfPaths(List<String> sourcePaths);

  Future<String?> chooseDestinationDirectory();

  Future<RenderedSplitPdfPage> renderFirstPage(String sourcePath);

  Stream<SplitPdfUpdate> split({
    required String sourcePath,
    required String destinationDirectory,
    required SplitPdfMode mode,
    required int? everyNPages,
    required String? splitAfterPages,
    required int? maximumSizeMb,
  });
}

class LocalSplitPdfWorkflow implements SplitPdfWorkflow {
  const LocalSplitPdfWorkflow();

  @override
  Future<SelectedSplitPdf?> selectPdf() async {
    const typeGroup = XTypeGroup(label: 'PDF document', extensions: ['pdf']);
    final selected = await openFile(acceptedTypeGroups: const [typeGroup]);
    if (selected == null) return null;
    return preparePdfPaths([selected.path]);
  }

  @override
  Future<SelectedSplitPdf> preparePdfPaths(List<String> sourcePaths) async {
    if (sourcePaths.length != 1) {
      throw const SplitPdfSelectionException(
        SplitPdfProblem(
          code: rust_application.ApplicationErrorCode.invalidRequest,
          message: 'Split PDF supports one PDF at a time.',
        ),
      );
    }
    final sourcePath = sourcePaths.single;
    final file = File(sourcePath);
    final filename = file.uri.pathSegments.isEmpty
        ? sourcePath
        : file.uri.pathSegments.last;
    if (!filename.toLowerCase().endsWith('.pdf')) {
      throw const SplitPdfSelectionException(
        SplitPdfProblem(
          code: rust_application.ApplicationErrorCode.invalidRequest,
          message: 'Only one PDF document is supported.',
        ),
      );
    }
    try {
      final info = await rust_split.inspectSplitPdfSource(
        sourcePath: sourcePath,
      );
      return SelectedSplitPdf(
        displayName: filename,
        sourcePath: sourcePath,
        sourceDirectory: file.parent.path,
        pageCount: info.pageCount,
        sizeBytes: info.sizeBytes.toInt(),
        hasWarnings: info.hasWarnings,
      );
    } on rust_application.ApplicationError catch (error) {
      throw SplitPdfSelectionException(
        SplitPdfProblem(code: error.code, message: error.message),
      );
    } on SplitPdfSelectionException {
      rethrow;
    } on Object {
      throw const SplitPdfSelectionException(
        SplitPdfProblem(
          code: rust_application.ApplicationErrorCode.internal,
          message: 'This PDF could not be opened.',
        ),
      );
    }
  }

  @override
  Future<String?> chooseDestinationDirectory() =>
      getDirectoryPath(confirmButtonText: 'Choose output folder');

  @override
  Future<RenderedSplitPdfPage> renderFirstPage(String sourcePath) async {
    final result = await rust_preview.renderPdfPage(
      request: rust_preview.RenderPdfPageRequest(
        sourcePath: sourcePath,
        pageIndex: 0,
        targetWidth: 1000,
        destinationPath: null,
      ),
    );
    return RenderedSplitPdfPage(outputPath: result.outputPath);
  }

  @override
  Stream<SplitPdfUpdate> split({
    required String sourcePath,
    required String destinationDirectory,
    required SplitPdfMode mode,
    required int? everyNPages,
    required String? splitAfterPages,
    required int? maximumSizeMb,
  }) async* {
    final updates = rust_split.splitPdf(
      request: rust_split.SplitPdfRequest(
        sourcePath: sourcePath,
        destinationDirectory: destinationDirectory,
        mode: switch (mode) {
          SplitPdfMode.everyPage => rust_split.SplitPdfMode.everyPage,
          SplitPdfMode.everyNPages => rust_split.SplitPdfMode.everyNPages,
          SplitPdfMode.splitAfterPages =>
            rust_split.SplitPdfMode.splitAfterPages,
          SplitPdfMode.maximumFileSize =>
            rust_split.SplitPdfMode.maximumFileSize,
        },
        everyNPages: everyNPages,
        splitAfterPages: splitAfterPages,
        maximumSizeMb: maximumSizeMb == null
            ? null
            : BigInt.from(maximumSizeMb),
      ),
    );
    await for (final update in updates) {
      yield SplitPdfUpdate(
        status: switch (update.status) {
          rust_split.SplitPdfStatus.running => SplitPdfUpdateStatus.running,
          rust_split.SplitPdfStatus.complete => SplitPdfUpdateStatus.complete,
          rust_split.SplitPdfStatus.failed => SplitPdfUpdateStatus.failed,
        },
        stage: switch (update.stage) {
          rust_split.SplitPdfStage.preparing => SplitPdfProgressStage.preparing,
          rust_split.SplitPdfStage.findingSplitPoints =>
            SplitPdfProgressStage.findingSplitPoints,
          rust_split.SplitPdfStage.creating => SplitPdfProgressStage.creating,
          rust_split.SplitPdfStage.validating =>
            SplitPdfProgressStage.validating,
          rust_split.SplitPdfStage.publishing =>
            SplitPdfProgressStage.publishing,
          rust_split.SplitPdfStage.completed => SplitPdfProgressStage.completed,
        },
        sourcePageCount: update.sourcePageCount,
        currentPart: update.currentPart,
        totalParts: update.totalParts,
        outputDirectory: update.outputDirectory,
        parts: update.parts
            .map(
              (part) => SplitPdfPart(
                outputPath: part.outputPath,
                range: SplitPdfRange(
                  firstPage: part.firstPage,
                  lastPage: part.lastPage,
                ),
                sizeBytes: part.sizeBytes.toInt(),
              ),
            )
            .toList(growable: false),
        hasWarnings: update.hasWarnings,
        failedPageNumber: update.failedPageNumber,
        actualSizeBytes: update.actualSizeBytes?.toInt(),
        limitSizeBytes: update.limitSizeBytes?.toInt(),
        error: update.error == null
            ? null
            : SplitPdfProblem(
                code: update.error!.code,
                message: update.error!.message,
              ),
      );
    }
  }
}

int? parsePositiveWholeNumber(String value) {
  final normalized = value.trim();
  if (!RegExp(r'^\d+$').hasMatch(normalized)) return null;
  final parsed = int.tryParse(normalized);
  return parsed != null && parsed > 0 ? parsed : null;
}

List<SplitPdfRange>? deriveDeterministicSplitRanges({
  required int pageCount,
  required SplitPdfMode mode,
  required String everyNPages,
  required String splitAfterPages,
}) {
  if (pageCount < 2) return null;
  switch (mode) {
    case SplitPdfMode.everyPage:
      return [
        for (var page = 1; page <= pageCount; page++)
          SplitPdfRange(firstPage: page, lastPage: page),
      ];
    case SplitPdfMode.everyNPages:
      final every = parsePositiveWholeNumber(everyNPages);
      if (every == null || every >= pageCount) return null;
      return [
        for (var first = 1; first <= pageCount; first += every)
          SplitPdfRange(
            firstPage: first,
            lastPage: (first + every - 1).clamp(1, pageCount),
          ),
      ];
    case SplitPdfMode.splitAfterPages:
      final tokens = splitAfterPages.split(',');
      if (splitAfterPages.trim().isEmpty ||
          tokens.any((token) => token.trim().isEmpty)) {
        return null;
      }
      final points = <int>[];
      for (final token in tokens) {
        final point = parsePositiveWholeNumber(token);
        if (point == null ||
            point >= pageCount ||
            (points.isNotEmpty && point <= points.last)) {
          return null;
        }
        points.add(point);
      }
      var first = 1;
      final ranges = <SplitPdfRange>[];
      for (final point in points) {
        ranges.add(SplitPdfRange(firstPage: first, lastPage: point));
        first = point + 1;
      }
      ranges.add(SplitPdfRange(firstPage: first, lastPage: pageCount));
      return ranges;
    case SplitPdfMode.maximumFileSize:
      return null;
  }
}
