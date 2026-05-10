import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/state/openpinch_controller.dart';
import '../../shared/theme.dart';

class DashboardPage extends ConsumerWidget {
  const DashboardPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.watch(openPinchControllerProvider);
    final theme = Theme.of(context);
    return ListView(
      children: <Widget>[
        Text('Runtime Overview', style: theme.textTheme.headlineSmall),
        const SizedBox(height: 8),
        Text(
          'Bundled sidecars, connector inventory, doctor findings, and active model profiles in one view.',
          style: theme.textTheme.bodyLarge,
        ),
        const SizedBox(height: 18),
        Wrap(
          spacing: 16,
          runSpacing: 16,
          children: <Widget>[
            _MetricCard(
              label: 'Host',
              value: controller.host['status']?.toString() ?? 'unknown',
              accent: context.palette.teal,
            ),
            _MetricCard(
              label: 'Runtime',
              value: controller.status['status']?.toString() ?? 'unknown',
              accent: context.palette.aqua,
            ),
            _MetricCard(
              label: 'Sessions',
              value: '${controller.sessions.length}',
              accent: context.palette.ember,
            ),
            _MetricCard(
              label: 'Pairings',
              value: '${controller.pairings.length}',
              accent: context.palette.wine,
            ),
          ],
        ),
        const SizedBox(height: 18),
        _Panel(
          title: 'Doctor Findings',
          child: Column(
            children: controller.doctorFindings
                .take(8)
                .map(
                  (finding) => ListTile(
                    contentPadding: EdgeInsets.zero,
                    leading: CircleAvatar(
                      backgroundColor: _severityColor(
                        context,
                        finding['severity']?.toString() ?? 'info',
                      ).withValues(alpha: 0.14),
                      child: Icon(
                        Icons.health_and_safety_rounded,
                        color: _severityColor(
                          context,
                          finding['severity']?.toString() ?? 'info',
                        ),
                      ),
                    ),
                    title: Text(finding['summary']?.toString() ?? 'finding'),
                    subtitle: Text(
                      finding['detail']?.toString() ?? '',
                    ),
                    trailing: Text(finding['status']?.toString() ?? ''),
                  ),
                )
                .toList(growable: false),
          ),
        ),
        const SizedBox(height: 18),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(
              child: _Panel(
                title: 'Connectors',
                child: Column(
                  children: controller.connectors
                      .map(
                        (connector) => ListTile(
                          contentPadding: EdgeInsets.zero,
                          leading: Icon(
                            connector['implemented'] == true
                                ? Icons.check_circle_rounded
                                : Icons.pending_outlined,
                            color: connector['implemented'] == true
                                ? context.palette.aqua
                                : context.palette.ember,
                          ),
                          title: Text(
                              connector['name']?.toString() ?? 'connector'),
                          subtitle: Text(
                            '${connector['mode'] ?? 'unknown'} • ${connector['health'] ?? 'unknown'}',
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
                title: 'Model Profiles',
                child: Column(
                  children: controller.models
                      .map(
                        (profile) => ListTile(
                          contentPadding: EdgeInsets.zero,
                          title: Text(profile['name']?.toString() ?? 'profile'),
                          subtitle: Text(
                            '${profile['mode'] ?? 'mode'} • ${(profile['provider_order'] as List<dynamic>? ?? const <dynamic>[]).join(", ")}',
                          ),
                          trailing: profile['default_profile'] == true
                              ? const Chip(label: Text('Default'))
                              : null,
                        ),
                      )
                      .toList(growable: false),
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _MetricCard extends StatelessWidget {
  const _MetricCard({
    required this.label,
    required this.value,
    required this.accent,
  });

  final String label;
  final String value;
  final Color accent;

  @override
  Widget build(BuildContext context) {
    return ConstrainedBox(
      constraints: const BoxConstraints(minWidth: 180),
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(18),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Container(
                width: 12,
                height: 12,
                decoration: BoxDecoration(
                  color: accent,
                  borderRadius: BorderRadius.circular(999),
                ),
              ),
              const SizedBox(height: 18),
              Text(label, style: Theme.of(context).textTheme.bodyMedium),
              const SizedBox(height: 6),
              Text(
                value,
                style: Theme.of(context).textTheme.headlineSmall,
              ),
            ],
          ),
        ),
      ),
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

Color _severityColor(BuildContext context, String severity) {
  return switch (severity) {
    'warning' => context.palette.ember,
    'error' => context.palette.wine,
    _ => context.palette.aqua,
  };
}

extension on OpenPinchController {
  List<Map<String, dynamic>> get doctorFindings {
    final findings = doctor['findings'];
    if (findings is List<dynamic>) {
      return findings.map<Map<String, dynamic>>((dynamic entry) {
        if (entry is Map<String, dynamic>) {
          return entry;
        }
        if (entry is Map) {
          return entry.map((key, value) => MapEntry('$key', value));
        }
        return <String, dynamic>{};
      }).toList(growable: false);
    }
    return <Map<String, dynamic>>[];
  }
}
