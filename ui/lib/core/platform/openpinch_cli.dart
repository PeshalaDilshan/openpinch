import 'dart:async';
import 'dart:convert';
import 'dart:io';

class OpenPinchCli {
  Future<void> ensureHostReady() async {
    final health = await desktopHealth();
    if (health['healthy'] == true) {
      return;
    }

    final binary = await resolveBinary();
    await Process.start(
      binary,
      const <String>['--json', 'desktop', 'host'],
      mode: ProcessStartMode.detached,
    );

    for (var attempt = 0; attempt < 24; attempt++) {
      await Future<void>.delayed(const Duration(milliseconds: 500));
      final next = await desktopHealth();
      if (next['healthy'] == true) {
        return;
      }
    }

    throw const ProcessException(
      'openpinch',
      <String>['desktop', 'host'],
      'desktop host did not become healthy in time',
    );
  }

  Future<Map<String, dynamic>> desktopHealth() => runJson(
        const <String>['desktop', 'health'],
        tolerateFailure: true,
      );

  Future<Map<String, dynamic>> desktopShutdown() =>
      runJson(const <String>['desktop', 'shutdown']);

  Future<Map<String, dynamic>> status() => runJson(const <String>['status']);

  Future<Map<String, dynamic>> connectorList() =>
      runJson(const <String>['connector', 'list']);

  Future<Map<String, dynamic>> sessionList() => runJson(
        const <String>['session', 'list', '--limit', '24'],
      );

  Future<Map<String, dynamic>> sessionShow(String sessionId) => runJson(
        <String>['session', 'show', sessionId, '--limit', '80'],
      );

  Future<Map<String, dynamic>> pairingList() => runJson(
        const <String>['pairing', 'list', '--limit', '24'],
      );

  Future<Map<String, dynamic>> approvePairing(String pairingId) => runJson(
        <String>['pairing', 'approve', pairingId],
      );

  Future<Map<String, dynamic>> revokePairing(String pairingId) => runJson(
        <String>['pairing', 'revoke', pairingId],
      );

  Future<Map<String, dynamic>> doctor() => runJson(const <String>['doctor']);

  Future<Map<String, dynamic>> modelProfiles() =>
      runJson(const <String>['model', 'profile']);

  Future<Map<String, dynamic>> brainSuggest() =>
      runJson(const <String>['brain', 'suggest', '--limit', '8']);

  Future<Map<String, dynamic>> brainTasks() => runJson(
        const <String>['brain', 'task', 'list', '--limit', '8'],
      );

  Future<Map<String, dynamic>> brainRecall(String query) => runJson(
        <String>['brain', 'recall', query, '--limit', '8'],
      );

  Future<Map<String, dynamic>> postDesktopMessage({
    required String sender,
    required String channelId,
    required String body,
  }) {
    return runJson(
      <String>[
        'message',
        'post',
        'desktop',
        channelId,
        body,
        '--sender',
        sender,
        '--metadata',
        '{"source":"desktop-app"}',
      ],
    );
  }

  Future<Map<String, dynamic>> runJson(
    List<String> args, {
    bool tolerateFailure = false,
  }) async {
    final binary = await resolveBinary();
    final result = await Process.run(binary, <String>['--json', ...args]);
    final stdout = result.stdout.toString().trim();
    final stderr = result.stderr.toString().trim();

    if (result.exitCode != 0 && !tolerateFailure) {
      throw ProcessException(
        binary,
        args,
        stderr.isNotEmpty ? stderr : stdout,
        result.exitCode,
      );
    }

    if (stdout.isEmpty) {
      return <String, dynamic>{
        'status': tolerateFailure ? 'unknown' : 'ok',
        'healthy': false,
      };
    }

    final decoded = jsonDecode(stdout);
    if (decoded is Map<String, dynamic>) {
      return decoded;
    }
    if (decoded is List<dynamic>) {
      return <String, dynamic>{'items': decoded};
    }
    return <String, dynamic>{'value': decoded};
  }

  Future<String> resolveBinary() async {
    final configured = Platform.environment['OPENPINCH_BIN'];
    if (configured != null && configured.isNotEmpty) {
      return configured;
    }

    final binaryName = Platform.isWindows ? 'openpinch.exe' : 'openpinch';
    final executable = File(Platform.resolvedExecutable);
    final candidates = <String>[
      '${executable.parent.path}${Platform.pathSeparator}$binaryName',
      '${executable.parent.parent.path}${Platform.pathSeparator}$binaryName',
      binaryName,
    ];

    for (final candidate in candidates) {
      if (candidate == binaryName || File(candidate).existsSync()) {
        return candidate;
      }
    }

    return binaryName;
  }
}
