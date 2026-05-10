import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/state/openpinch_controller.dart';
import '../../shared/theme.dart';

class OperationsPage extends ConsumerWidget {
  const OperationsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.watch(openPinchControllerProvider);
    return ListView(
      children: <Widget>[
        Text('Operations', style: Theme.of(context).textTheme.headlineSmall),
        const SizedBox(height: 8),
        Text(
          'Pairing approvals, connector readiness, and runtime operational state for the bundled desktop environment.',
          style: Theme.of(context).textTheme.bodyLarge,
        ),
        const SizedBox(height: 18),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(
              child: _Panel(
                title: 'Pending Pairings',
                child: Column(
                  children: controller.pairings
                      .map(
                        (pairing) => Card(
                          color: Colors.transparent,
                          child: Padding(
                            padding: const EdgeInsets.all(16),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: <Widget>[
                                Text(
                                  pairing['sender']?.toString() ?? 'sender',
                                  style: Theme.of(context).textTheme.titleLarge,
                                ),
                                const SizedBox(height: 6),
                                Text(
                                  '${pairing['connector'] ?? 'connector'} • ${pairing['reason'] ?? ''}',
                                ),
                                const SizedBox(height: 12),
                                Row(
                                  children: <Widget>[
                                    ElevatedButton(
                                      onPressed: () =>
                                          controller.approvePairing(
                                        pairing['id']?.toString() ?? '',
                                      ),
                                      child: const Text('Approve'),
                                    ),
                                    const SizedBox(width: 10),
                                    OutlinedButton(
                                      onPressed: () => controller.revokePairing(
                                        pairing['id']?.toString() ?? '',
                                      ),
                                      child: const Text('Revoke'),
                                    ),
                                  ],
                                ),
                              ],
                            ),
                          ),
                        ),
                      )
                      .toList(growable: false),
                ),
              ),
            ),
            const SizedBox(width: 18),
            Expanded(
              child: _Panel(
                title: 'Connector Health',
                child: Column(
                  children: controller.connectors
                      .map(
                        (connector) => ListTile(
                          contentPadding: EdgeInsets.zero,
                          leading: CircleAvatar(
                            backgroundColor: (connector['implemented'] == true
                                    ? context.palette.aqua
                                    : context.palette.ember)
                                .withValues(alpha: 0.14),
                            child: Icon(
                              connector['implemented'] == true
                                  ? Icons.usb_rounded
                                  : Icons.hourglass_bottom_rounded,
                              color: connector['implemented'] == true
                                  ? context.palette.aqua
                                  : context.palette.ember,
                            ),
                          ),
                          title: Text(connector['name']?.toString() ?? ''),
                          subtitle: Text(
                            '${connector['mode'] ?? 'mode'} • ${connector['health'] ?? 'health'}',
                          ),
                        ),
                      )
                      .toList(growable: false),
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 18),
        _Panel(
          title: 'Operator Notes',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                'Desktop host status: ${controller.host['status'] ?? 'unknown'}',
              ),
              const SizedBox(height: 8),
              Text(
                  'Gateway endpoint: ${controller.status['gateway'] ?? controller.host['gateway_endpoint'] ?? 'n/a'}'),
              const SizedBox(height: 8),
              Text(
                  'Log file: ${controller.host['host']?['log_file'] ?? controller.host['log_file'] ?? 'n/a'}'),
              const SizedBox(height: 12),
              Wrap(
                spacing: 10,
                runSpacing: 10,
                children: <Widget>[
                  ElevatedButton.icon(
                    onPressed: controller.refresh,
                    icon: const Icon(Icons.refresh_rounded),
                    label: const Text('Refresh'),
                  ),
                  OutlinedButton.icon(
                    onPressed: controller.restartHost,
                    icon: const Icon(Icons.replay_rounded),
                    label: const Text('Restart Host'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _Panel extends StatelessWidget {
  const _Panel({required this.title, required this.child});

  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(title, style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 12),
            child,
          ],
        ),
      ),
    );
  }
}
