import 'package:flutter_test/flutter_test.dart';
import 'package:ai_trading_mobile/main.dart';

void main() {
  testWidgets('App smoke test', (WidgetTester tester) async {
    // Verify the app launches without crashing
    await tester.pumpWidget(const AiTradingApp());
    expect(find.text('AI Trading'), findsOneWidget);
  });
}
