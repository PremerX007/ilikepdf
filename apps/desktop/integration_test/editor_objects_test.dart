import 'package:flutter_test/flutter_test.dart';
import 'package:ilikepdf/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

import '../test/editor_object_scenarios.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustLib.init);
  editorObjectScenarios();
}
