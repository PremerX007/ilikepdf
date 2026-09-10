import 'package:file_selector/file_selector.dart';
import 'package:ilikepdf/src/rust/api/pdf_preview.dart' as rust;

class SelectedPdf {
  const SelectedPdf({
    required this.displayName,
    required this.sourcePath,
    required this.pageCount,
    required this.firstPageWidthPoints,
    required this.firstPageHeightPoints,
  });

  final String displayName;
  final String sourcePath;
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

abstract interface class PdfPreviewWorkflow {
  Future<SelectedPdf?> selectAndInspect();

  Future<RenderedPdfPage> renderFirstPage(String sourcePath);
}

class LocalPdfPreviewWorkflow implements PdfPreviewWorkflow {
  const LocalPdfPreviewWorkflow();

  @override
  Future<SelectedPdf?> selectAndInspect() async {
    const typeGroup = XTypeGroup(label: 'PDF documents', extensions: ['pdf']);
    final selected = await openFile(acceptedTypeGroups: const [typeGroup]);
    if (selected == null) {
      return null;
    }

    final info = await rust.openPdfDocument(sourcePath: selected.path);
    return SelectedPdf(
      displayName: selected.name,
      sourcePath: selected.path,
      pageCount: info.pageCount,
      firstPageWidthPoints: info.firstPageSize?.widthPoints,
      firstPageHeightPoints: info.firstPageSize?.heightPoints,
    );
  }

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) async {
    final result = await rust.renderPdfPage(
      request: rust.RenderPdfPageRequest(
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
