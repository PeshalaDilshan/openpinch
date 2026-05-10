import 'dart:ui' show Size;

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:openpinch_ui/app/app.dart';
import 'package:openpinch_ui/core/platform/openpinch_cli.dart';
import 'package:openpinch_ui/core/state/openpinch_controller.dart';

class FakeOpenPinchCli extends OpenPinchCli {
  @override
  Future<void> ensureHostReady() async {}

  @override
  Future<Map<String, dynamic>> desktopHealth() async => <String, dynamic>{
        'status': 'running',
        'healthy': true,
        'host': <String, dynamic>{'gateway_endpoint': '127.0.0.1:50051'},
      };

  @override
  Future<Map<String, dynamic>> status() async => <String, dynamic>{
        'status': 'ready',
        'gateway': '127.0.0.1:50051',
      };

  @override
  Future<Map<String, dynamic>> connectorList() async => <String, dynamic>{
        'connectors': <Map<String, dynamic>>[
          <String, dynamic>{
            'name': 'desktop',
            'implemented': true,
            'mode': 'native',
            'health': 'ready',
          },
        ],
      };

  @override
  Future<Map<String, dynamic>> sessionList() async => <String, dynamic>{
        'sessions': <Map<String, dynamic>>[
          <String, dynamic>{
            'id': 'session-1',
            'title': 'Desktop Session',
            'connector': 'desktop',
            'status': 'active',
            'last_message_preview': 'hello',
          },
        ],
      };

  @override
  Future<Map<String, dynamic>> sessionShow(String sessionId) async =>
      <String, dynamic>{
        'session': <String, dynamic>{
          'id': sessionId,
          'title': 'Desktop Session',
          'participant': 'desktop-user',
          'channel_id': 'desktop',
        },
        'messages': <Map<String, dynamic>>[],
      };

  @override
  Future<Map<String, dynamic>> pairingList() async =>
      <String, dynamic>{'pairings': <dynamic>[]};

  @override
  Future<Map<String, dynamic>> doctor() async => <String, dynamic>{
        'status': 'ready',
        'findings': <Map<String, dynamic>>[],
      };

  @override
  Future<Map<String, dynamic>> modelProfiles() async =>
      <String, dynamic>{'profiles': <dynamic>[]};

  @override
  Future<Map<String, dynamic>> brainSuggest() async =>
      <String, dynamic>{'suggestions': <dynamic>[]};

  @override
  Future<Map<String, dynamic>> brainTasks() async =>
      <String, dynamic>{'tasks': <dynamic>[]};
}

void main() {
  testWidgets('renders desktop shell title', (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1600, 1000);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);

    await tester.pumpWidget(
      ProviderScope(
        overrides: <Override>[
          openPinchCliProvider.overrideWithValue(FakeOpenPinchCli()),
        ],
        child: const OpenPinchDesktopApp(),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('OpenPinch Desktop'), findsOneWidget);
    expect(find.text('Dashboard'), findsWidgets);
  });
}
