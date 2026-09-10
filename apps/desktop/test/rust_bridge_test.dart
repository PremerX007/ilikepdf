import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/rust/api/application.dart';
import 'package:ilikepdf/src/rust/frb_generated.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  test('returns typed application information from Rust', () async {
    final info = await getApplicationInfo();

    expect(info.name, 'ilikepdf');
    expect(info.version, '0.1.0');
    expect(info.localOnly, isTrue);
  });
}
