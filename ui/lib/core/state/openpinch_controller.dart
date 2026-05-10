import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../platform/openpinch_cli.dart';

final openPinchCliProvider = Provider<OpenPinchCli>((ref) {
  return OpenPinchCli();
});

final openPinchControllerProvider =
    ChangeNotifierProvider<OpenPinchController>((ref) {
  final controller = OpenPinchController(ref.watch(openPinchCliProvider));
  return controller;
});

class OpenPinchController extends ChangeNotifier {
  OpenPinchController(this._cli);

  final OpenPinchCli _cli;

  Timer? _poller;
  bool _bootstrapped = false;

  bool booting = false;
  bool refreshing = false;
  String lastError = '';
  String actionMessage = '';

  Map<String, dynamic> host = <String, dynamic>{};
  Map<String, dynamic> status = <String, dynamic>{};
  Map<String, dynamic> doctor = <String, dynamic>{'findings': <dynamic>[]};
  List<Map<String, dynamic>> connectors = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> sessions = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> messages = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> pairings = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> models = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> suggestions = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> tasks = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> recallEntities = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> recallTasks = <Map<String, dynamic>>[];
  String recallSummary = '';
  String? selectedSessionId;
  Map<String, dynamic>? selectedSession;

  Future<void> bootstrap() async {
    if (_bootstrapped) {
      return;
    }
    _bootstrapped = true;
    booting = true;
    notifyListeners();

    try {
      await _cli.ensureHostReady();
      await refresh();
      _poller = Timer.periodic(const Duration(seconds: 6), (_) {
        unawaited(refresh(silent: true));
      });
    } catch (error) {
      lastError = '$error';
    } finally {
      booting = false;
      notifyListeners();
    }
  }

  Future<void> refresh({bool silent = false}) async {
    if (refreshing) {
      return;
    }
    refreshing = true;
    if (!silent) {
      notifyListeners();
    }

    try {
      final results = await Future.wait<dynamic>(<Future<dynamic>>[
        _cli.desktopHealth(),
        _cli.status(),
        _cli.connectorList(),
        _cli.sessionList(),
        _cli.pairingList(),
        _cli.doctor(),
        _cli.modelProfiles(),
        _cli.brainSuggest(),
        _cli.brainTasks(),
      ]);

      host = _asMap(results[0]);
      status = _asMap(results[1]);
      connectors = _asMapList(_asMap(results[2])['connectors']);
      final sessionPayload = _asMap(results[3]);
      sessions = _asMapList(sessionPayload['sessions']);
      pairings = _asMapList(_asMap(results[4])['pairings']);
      doctor = _asMap(results[5]);
      models = _asMapList(_asMap(results[6])['profiles']);
      suggestions = _asMapList(_asMap(results[7])['suggestions']);
      tasks = _asMapList(_asMap(results[8])['tasks']);

      if (selectedSessionId == null && sessions.isNotEmpty) {
        selectedSessionId = _stringValue(sessions.first['id']);
      }

      if (selectedSessionId != null && selectedSessionId!.isNotEmpty) {
        final detail = await _cli.sessionShow(selectedSessionId!);
        selectedSession = _asNullableMap(detail['session']);
        messages = _asMapList(detail['messages']);
      } else {
        selectedSession = null;
        messages = <Map<String, dynamic>>[];
      }
      lastError = '';
    } catch (error) {
      lastError = '$error';
    } finally {
      refreshing = false;
      notifyListeners();
    }
  }

  Future<void> selectSession(String sessionId) async {
    selectedSessionId = sessionId;
    notifyListeners();
    await refresh(silent: true);
  }

  Future<void> sendDesktopMessage({
    required String sender,
    required String channelId,
    required String body,
  }) async {
    final result = await _cli.postDesktopMessage(
      sender: sender,
      channelId: channelId,
      body: body,
    );
    selectedSessionId = _stringValue(result['session_id']);
    actionMessage = _stringValue(result['delivery_state']);
    await refresh();
  }

  Future<void> recallBrain(String query) async {
    final result = await _cli.brainRecall(query);
    recallSummary = _stringValue(result['summary']);
    recallEntities = _asMapList(result['entities']);
    recallTasks = _asMapList(result['tasks']);
    notifyListeners();
  }

  Future<void> approvePairing(String pairingId) async {
    await _cli.approvePairing(pairingId);
    actionMessage = 'Pairing approved';
    await refresh();
  }

  Future<void> revokePairing(String pairingId) async {
    await _cli.revokePairing(pairingId);
    actionMessage = 'Pairing revoked';
    await refresh();
  }

  Future<void> shutdownHost() async {
    await _cli.desktopShutdown();
    _poller?.cancel();
    _poller = null;
    _bootstrapped = false;
    await refresh();
  }

  Future<void> restartHost() async {
    _poller?.cancel();
    _poller = null;
    _bootstrapped = false;
    await bootstrap();
  }

  @override
  void dispose() {
    _poller?.cancel();
    super.dispose();
  }
}

Map<String, dynamic> _asMap(dynamic value) {
  if (value is Map<String, dynamic>) {
    return value;
  }
  if (value is Map) {
    return value.map((key, entry) => MapEntry('$key', entry));
  }
  return <String, dynamic>{};
}

Map<String, dynamic>? _asNullableMap(dynamic value) {
  final map = _asMap(value);
  return map.isEmpty ? null : map;
}

List<Map<String, dynamic>> _asMapList(dynamic value) {
  if (value is List<dynamic>) {
    return value.map<Map<String, dynamic>>(_asMap).toList(growable: false);
  }
  return <Map<String, dynamic>>[];
}

String _stringValue(dynamic value) {
  if (value == null) {
    return '';
  }
  return '$value';
}
