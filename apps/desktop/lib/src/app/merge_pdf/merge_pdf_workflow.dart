import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/rust/api/error.dart' as rust_application;
import 'package:ilikepdf/src/rust/api/merge_pdf.dart' as rust_merge;
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust_preview;

class SelectedMergePdf {
  const SelectedMergePdf({
    required this.displayName,
    required this.sourcePath,
    required this.sourceDirectory,
    required this.pageCount,
    this.inspectionProblem,
  });

  final String displayName;
  final String sourcePath;
  final String sourceDirectory;
  final int pageCount;
  final MergePdfProblem? inspectionProblem;
}

class RenderedMergePdfPage {
  const RenderedMergePdfPage({required this.outputPath});

  final String outputPath;
}

enum MergePdfProgressStage {
  preparing,
  merging,
  validating,
  publishing,
  completed,
}

enum MergePdfUpdateStatus { running, complete, failed }

class MergePdfProblem {
  const MergePdfProblem({required this.code, required this.message});

  final rust_application.ApplicationErrorCode code;
  final String message;
}

class MergePdfSelectionException implements Exception {
  const MergePdfSelectionException(this.problem);

  final MergePdfProblem problem;
}

class MergePdfUpdate {
  const MergePdfUpdate({
    required this.status,
    required this.stage,
    required this.inputCount,
    required this.totalPageCount,
    required this.outputPath,
    required this.warningInputCount,
    required this.hasWarnings,
    required this.failedInputIndex,
    required this.failedInputPath,
    required this.error,
  });

  final MergePdfUpdateStatus status;
  final MergePdfProgressStage stage;
  final int inputCount;
  final int totalPageCount;
  final String? outputPath;
  final int warningInputCount;
  final bool hasWarnings;
  final int? failedInputIndex;
  final String? failedInputPath;
  final MergePdfProblem? error;
}

abstract interface class MergePdfWorkflow {
  Future<List<SelectedMergePdf>> selectPdfs();

  Future<List<SelectedMergePdf>> preparePdfPaths(List<String> sourcePaths);

  Future<String?> chooseDestinationDirectory();

  Future<RenderedMergePdfPage> renderFirstPage(String sourcePath);

  Stream<MergePdfUpdate> merge({
    required List<String> sourcePaths,
    required String destinationDirectory,
    required String outputName,
  });
}

class LocalMergePdfWorkflow implements MergePdfWorkflow {
  const LocalMergePdfWorkflow();

  @override
  Future<List<SelectedMergePdf>> selectPdfs() async {
    const typeGroup = XTypeGroup(label: 'PDF documents', extensions: ['pdf']);
    final selected = await openFiles(acceptedTypeGroups: const [typeGroup]);
    return preparePdfPaths(
      selected.map((file) => file.path).toList(growable: false),
    );
  }

  @override
  Future<List<SelectedMergePdf>> preparePdfPaths(
    List<String> sourcePaths,
  ) async {
    final selected = <SelectedMergePdf>[];
    for (final sourcePath in sourcePaths) {
      final file = File(sourcePath);
      final filename = file.uri.pathSegments.isEmpty
          ? sourcePath
          : file.uri.pathSegments.last;
      if (!filename.toLowerCase().endsWith('.pdf')) {
        throw const MergePdfSelectionException(
          MergePdfProblem(
            code: rust_application.ApplicationErrorCode.invalidRequest,
            message: 'Only PDF documents are supported.',
          ),
        );
      }
      try {
        final info = await rust_preview.openPdfDocument(sourcePath: sourcePath);
        selected.add(
          SelectedMergePdf(
            displayName: filename,
            sourcePath: sourcePath,
            sourceDirectory: file.parent.path,
            pageCount: info.pageCount,
          ),
        );
      } on rust_application.ApplicationError catch (error) {
        selected.add(
          SelectedMergePdf(
            displayName: filename,
            sourcePath: sourcePath,
            sourceDirectory: file.parent.path,
            pageCount: 0,
            inspectionProblem: MergePdfProblem(
              code: error.code,
              message: error.message,
            ),
          ),
        );
      } on Object {
        selected.add(
          SelectedMergePdf(
            displayName: filename,
            sourcePath: sourcePath,
            sourceDirectory: file.parent.path,
            pageCount: 0,
            inspectionProblem: const MergePdfProblem(
              code: rust_application.ApplicationErrorCode.internal,
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
      getDirectoryPath(confirmButtonText: 'Choose output folder');

  @override
  Future<RenderedMergePdfPage> renderFirstPage(String sourcePath) async {
    final result = await rust_preview.renderPdfPage(
      request: rust_preview.RenderPdfPageRequest(
        sourcePath: sourcePath,
        pageIndex: 0,
        targetWidth: 1000,
        destinationPath: null,
      ),
    );
    return RenderedMergePdfPage(outputPath: result.outputPath);
  }

  @override
  Stream<MergePdfUpdate> merge({
    required List<String> sourcePaths,
    required String destinationDirectory,
    required String outputName,
  }) async* {
    final updates = rust_merge.mergePdf(
      request: rust_merge.MergePdfRequest(
        sourcePaths: sourcePaths,
        destinationDirectory: destinationDirectory,
        outputName: outputName,
      ),
    );
    await for (final update in updates) {
      yield MergePdfUpdate(
        status: switch (update.status) {
          rust_merge.MergePdfStatus.running => MergePdfUpdateStatus.running,
          rust_merge.MergePdfStatus.complete => MergePdfUpdateStatus.complete,
          rust_merge.MergePdfStatus.failed => MergePdfUpdateStatus.failed,
        },
        stage: switch (update.stage) {
          rust_merge.MergePdfStage.preparing => MergePdfProgressStage.preparing,
          rust_merge.MergePdfStage.merging => MergePdfProgressStage.merging,
          rust_merge.MergePdfStage.validating =>
            MergePdfProgressStage.validating,
          rust_merge.MergePdfStage.publishing =>
            MergePdfProgressStage.publishing,
          rust_merge.MergePdfStage.completed => MergePdfProgressStage.completed,
        },
        inputCount: update.inputCount,
        totalPageCount: update.totalPageCount,
        outputPath: update.outputPath,
        warningInputCount: update.warningInputCount,
        hasWarnings: update.hasWarnings,
        failedInputIndex: update.failedInputIndex,
        failedInputPath: update.failedInputPath,
        error: update.error == null
            ? null
            : MergePdfProblem(
                code: update.error!.code,
                message: update.error!.message,
              ),
      );
    }
  }
}

String? normalizeMergeOutputName(String value) {
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
