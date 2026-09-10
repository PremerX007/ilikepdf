import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/rust/api/application.dart';
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets('Flutter receives typed application information from Rust', (
    WidgetTester tester,
  ) async {
    final info = await getApplicationInfo();

    expect(info.name, 'ilikepdf');
    expect(info.version, '0.1.0');
    expect(info.localOnly, isTrue);
  });
}
