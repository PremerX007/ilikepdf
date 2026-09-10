import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/app.dart';
import 'package:ilikepdf/src/app/pdf_preview/pdf_preview_workflow.dart';

class FakePdfPreviewWorkflow implements PdfPreviewWorkflow {
  const FakePdfPreviewWorkflow();

  @override
  Future<SelectedPdf?> selectAndInspect() async => const SelectedPdf(
    displayName: 'fixture.pdf',
    sourcePath: r'C:\fixtures\fixture.pdf',
    pageCount: 2,
    firstPageWidthPoints: 300,
    firstPageHeightPoints: 200,
  );

  @override
  Future<RenderedPdfPage> renderFirstPage(String sourcePath) async =>
      throw UnimplementedError();
}

void main() {
  testWidgets('shows core version and local-only status', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const IlikepdfApp(
        applicationName: 'ilikepdf',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfWorkflow: FakePdfPreviewWorkflow(),
      ),
    );

    expect(find.text('ilikepdf'), findsOneWidget);
    expect(
      find.textContaining('Privacy mode: local processing only'),
      findsOneWidget,
    );
    expect(find.textContaining('Rust core 0.1.0'), findsOneWidget);
    expect(find.text('Local PDF preview'), findsOneWidget);
  });

  testWidgets('selects a PDF and shows page information', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      IlikepdfApp(
        applicationName: 'ilikepdf',
        coreVersion: '0.1.0',
        localOnly: true,
        pdfWorkflow: FakePdfPreviewWorkflow(),
      ),
    );

    await tester.tap(find.text('Select PDF'));
    await tester.pumpAndSettle();

    expect(find.text('fixture.pdf'), findsOneWidget);
    expect(find.text('2 pages'), findsOneWidget);
    expect(find.text('First page: 300 × 200 points'), findsOneWidget);
    expect(find.text('Render page 1'), findsOneWidget);
  });
}
