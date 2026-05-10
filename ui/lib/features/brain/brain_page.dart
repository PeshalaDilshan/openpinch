import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/state/openpinch_controller.dart';
import '../../shared/theme.dart';

class BrainPage extends ConsumerStatefulWidget {
  const BrainPage({super.key});

  @override
  ConsumerState<BrainPage> createState() => _BrainPageState();
}

class _BrainPageState extends ConsumerState<BrainPage> {
  final _queryController = TextEditingController();

  @override
  void dispose() {
    _queryController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = ref.watch(openPinchControllerProvider);
    return ListView(
      children: <Widget>[
        Text('Brain & Task Memory',
            style: Theme.of(context).textTheme.headlineSmall),
        const SizedBox(height: 8),
        Text(
          'Semantic recall, suggested next actions, and task state driven by the engine brain subsystem.',
          style: Theme.of(context).textTheme.bodyLarge,
        ),
        const SizedBox(height: 18),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(20),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text('Recall Search',
                    style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 12),
                Row(
                  children: <Widget>[
                    Expanded(
                      child: TextField(
                        controller: _queryController,
                        decoration: const InputDecoration(
                          labelText: 'Query',
                          hintText:
                              'Recent tasks, teammates, workspace, deployment, blockers...',
                        ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    ElevatedButton.icon(
                      onPressed: () =>
                          controller.recallBrain(_queryController.text.trim()),
                      icon: const Icon(Icons.search_rounded),
                      label: const Text('Recall'),
                    ),
                  ],
                ),
                if (controller.recallSummary.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 12),
                  Text(controller.recallSummary),
                ],
              ],
            ),
          ),
        ),
        const SizedBox(height: 18),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(
              child: _BrainPanel(
                title: 'Suggested Next Actions',
                child: Column(
                  children: controller.suggestions
                      .map(
                        (suggestion) => ListTile(
                          contentPadding: EdgeInsets.zero,
                          leading: CircleAvatar(
                            backgroundColor:
                                context.palette.aqua.withValues(alpha: 0.14),
                            child: Icon(
                              Icons.auto_awesome_rounded,
                              color: context.palette.aqua,
                            ),
                          ),
                          title: Text(suggestion['summary']?.toString() ?? ''),
                          subtitle:
                              Text(suggestion['reason']?.toString() ?? ''),
                          trailing: Text(
                            suggestion['score']?.toString() ?? '',
                          ),
                        ),
                      )
                      .toList(growable: false),
                ),
              ),
            ),
            const SizedBox(width: 18),
            Expanded(
              child: _BrainPanel(
                title: 'Tracked Tasks',
                child: Column(
                  children: controller.tasks
                      .map(
                        (task) => ListTile(
                          contentPadding: EdgeInsets.zero,
                          title: Text(task['summary']?.toString() ?? ''),
                          subtitle: Text(task['title']?.toString() ?? ''),
                          trailing: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            crossAxisAlignment: CrossAxisAlignment.end,
                            children: <Widget>[
                              Chip(
                                  label:
                                      Text(task['status']?.toString() ?? '')),
                              Text(task['priority']?.toString() ?? ''),
                            ],
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
        _BrainPanel(
          title: 'Recall Results',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              ...controller.recallEntities.map(
                (entity) => ListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(entity['title']?.toString() ?? ''),
                  subtitle: Text(entity['content']?.toString() ?? ''),
                  trailing: Text(entity['kind']?.toString() ?? ''),
                ),
              ),
              ...controller.recallTasks.map(
                (task) => ListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(task['summary']?.toString() ?? ''),
                  subtitle: Text(task['status']?.toString() ?? ''),
                  trailing: Text(task['priority']?.toString() ?? ''),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _BrainPanel extends StatelessWidget {
  const _BrainPanel({required this.title, required this.child});

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
