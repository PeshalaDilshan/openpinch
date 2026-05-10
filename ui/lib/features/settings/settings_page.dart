import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/state/openpinch_controller.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.watch(openPinchControllerProvider);
    final host = controller.host['host'] is Map<String, dynamic>
        ? controller.host['host'] as Map<String, dynamic>
        : controller.host;

    return ListView(
      children: <Widget>[
        Text('Settings & Host Control',
            style: Theme.of(context).textTheme.headlineSmall),
        const SizedBox(height: 8),
        Text(
          'Desktop profile, bundled runtime controls, and local operator details.',
          style: Theme.of(context).textTheme.bodyLarge,
        ),
        const SizedBox(height: 18),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text('Host Status',
                    style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 12),
                Text('State: ${controller.host['status'] ?? 'unknown'}'),
                const SizedBox(height: 8),
                Text('PID: ${host['host_pid'] ?? 'n/a'}'),
                const SizedBox(height: 8),
                Text('Gateway: ${host['gateway_endpoint'] ?? 'n/a'}'),
                const SizedBox(height: 8),
                Text('Runtime: ${host['runtime_endpoint'] ?? 'n/a'}'),
                const SizedBox(height: 8),
                Text('Logs: ${host['log_file'] ?? 'n/a'}'),
                const SizedBox(height: 16),
                Wrap(
                  spacing: 10,
                  runSpacing: 10,
                  children: <Widget>[
                    ElevatedButton.icon(
                      onPressed: controller.restartHost,
                      icon: const Icon(Icons.play_circle_fill_rounded),
                      label: const Text('Start / Restart'),
                    ),
                    OutlinedButton.icon(
                      onPressed: controller.shutdownHost,
                      icon: const Icon(Icons.stop_circle_outlined),
                      label: const Text('Shutdown'),
                    ),
                    OutlinedButton.icon(
                      onPressed: controller.refresh,
                      icon: const Icon(Icons.refresh_rounded),
                      label: const Text('Reload'),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 18),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text('Current Limits',
                    style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 12),
                Text('Connectors loaded: ${controller.connectors.length}'),
                const SizedBox(height: 8),
                Text('Sessions tracked: ${controller.sessions.length}'),
                const SizedBox(height: 8),
                Text('Brain suggestions: ${controller.suggestions.length}'),
                const SizedBox(height: 8),
                Text(
                    'Last error: ${controller.lastError.isEmpty ? 'none' : controller.lastError}'),
              ],
            ),
          ),
        ),
      ],
    );
  }
}
