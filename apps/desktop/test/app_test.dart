import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/app/app.dart';

void main() {
  testWidgets('shows core version and local-only status', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const IlikepdfApp(
        applicationName: 'ilikepdf',
        coreVersion: '0.1.0',
        localOnly: true,
      ),
    );

    expect(find.text('ilikepdf'), findsOneWidget);
    expect(find.text('Privacy mode: local processing only'), findsOneWidget);
    expect(find.text('Rust core 0.1.0'), findsOneWidget);
  });
}
