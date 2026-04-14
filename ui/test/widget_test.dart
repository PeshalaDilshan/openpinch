import 'package:flutter_test/flutter_test.dart';
import 'package:openpinch_ui/main.dart';

void main() {
  testWidgets('renders control title', (WidgetTester tester) async {
    await tester.pumpWidget(const OpenPinchApp());
    expect(find.text('OpenPinch Control'), findsOneWidget);
  });
}
