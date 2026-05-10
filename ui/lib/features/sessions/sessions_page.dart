import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/state/openpinch_controller.dart';
import '../../shared/theme.dart';

class SessionsPage extends ConsumerStatefulWidget {
  const SessionsPage({super.key});

  @override
  ConsumerState<SessionsPage> createState() => _SessionsPageState();
}

class _SessionsPageState extends ConsumerState<SessionsPage> {
  final _senderController = TextEditingController(text: 'desktop-user');
  final _channelController = TextEditingController(text: 'desktop');
  final _messageController = TextEditingController();

  @override
  void dispose() {
    _senderController.dispose();
    _channelController.dispose();
    _messageController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = ref.watch(openPinchControllerProvider);
    final selectedSession = controller.selectedSession;
    if (selectedSession != null) {
      _channelController.text =
          selectedSession['channel_id']?.toString() ?? _channelController.text;
    }

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        SizedBox(
          width: 320,
          child: Card(
            child: ListView.separated(
              padding: const EdgeInsets.all(16),
              itemCount: controller.sessions.length,
              separatorBuilder: (_, __) => const SizedBox(height: 10),
              itemBuilder: (context, index) {
                final session = controller.sessions[index];
                final sessionId = session['id']?.toString() ?? '';
                final selected = controller.selectedSessionId == sessionId;
                return InkWell(
                  borderRadius: BorderRadius.circular(20),
                  onTap: () => controller.selectSession(sessionId),
                  child: AnimatedContainer(
                    duration: const Duration(milliseconds: 180),
                    padding: const EdgeInsets.all(16),
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(20),
                      color: selected
                          ? context.palette.aqua.withValues(alpha: 0.12)
                          : Colors.transparent,
                      border: Border.all(
                        color: selected
                            ? context.palette.aqua
                            : context.palette.mist.withValues(alpha: 0.7),
                      ),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          session['title']?.toString() ?? 'Session',
                          style: Theme.of(context).textTheme.titleLarge,
                        ),
                        const SizedBox(height: 6),
                        Text(
                          session['last_message_preview']?.toString() ?? '',
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                        ),
                        const SizedBox(height: 10),
                        Wrap(
                          spacing: 8,
                          runSpacing: 8,
                          children: <Widget>[
                            Chip(
                                label: Text(
                                    session['connector']?.toString() ?? '')),
                            Chip(
                                label:
                                    Text(session['status']?.toString() ?? '')),
                          ],
                        ),
                      ],
                    ),
                  ),
                );
              },
            ),
          ),
        ),
        const SizedBox(width: 18),
        Expanded(
          child: Column(
            children: <Widget>[
              Expanded(
                child: Card(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Padding(
                        padding: const EdgeInsets.all(20),
                        child: Row(
                          children: <Widget>[
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: <Widget>[
                                  Text(
                                    selectedSession?['title']?.toString() ??
                                        'Local Desktop Session',
                                    style: Theme.of(context)
                                        .textTheme
                                        .headlineSmall,
                                  ),
                                  const SizedBox(height: 6),
                                  Text(
                                    selectedSession?['participant']
                                            ?.toString() ??
                                        'Use the composer below to create or continue a desktop session.',
                                  ),
                                ],
                              ),
                            ),
                            if (controller.actionMessage.isNotEmpty)
                              Chip(label: Text(controller.actionMessage)),
                          ],
                        ),
                      ),
                      const Divider(height: 1),
                      Expanded(
                        child: ListView.separated(
                          padding: const EdgeInsets.all(20),
                          itemCount: controller.messages.length,
                          separatorBuilder: (_, __) =>
                              const SizedBox(height: 12),
                          itemBuilder: (context, index) {
                            final message = controller.messages[index];
                            final isAssistant =
                                message['role']?.toString() == 'assistant';
                            return Align(
                              alignment: isAssistant
                                  ? Alignment.centerLeft
                                  : Alignment.centerRight,
                              child: ConstrainedBox(
                                constraints:
                                    const BoxConstraints(maxWidth: 720),
                                child: Container(
                                  padding: const EdgeInsets.all(16),
                                  decoration: BoxDecoration(
                                    borderRadius: BorderRadius.circular(22),
                                    color: isAssistant
                                        ? context.palette.mist
                                            .withValues(alpha: 0.7)
                                        : context.palette.aqua
                                            .withValues(alpha: 0.12),
                                  ),
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: <Widget>[
                                      Text(
                                        message['sender']?.toString() ??
                                            (isAssistant
                                                ? 'assistant'
                                                : 'user'),
                                        style: Theme.of(context)
                                            .textTheme
                                            .bodyMedium
                                            ?.copyWith(
                                                fontWeight: FontWeight.w700),
                                      ),
                                      const SizedBox(height: 6),
                                      Text(message['body']?.toString() ?? ''),
                                    ],
                                  ),
                                ),
                              ),
                            );
                          },
                        ),
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
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Expanded(
                            child: TextField(
                              controller: _senderController,
                              decoration: const InputDecoration(
                                labelText: 'Sender',
                              ),
                            ),
                          ),
                          const SizedBox(width: 12),
                          Expanded(
                            child: TextField(
                              controller: _channelController,
                              decoration: const InputDecoration(
                                labelText: 'Channel ID',
                              ),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      TextField(
                        controller: _messageController,
                        minLines: 3,
                        maxLines: 6,
                        decoration: const InputDecoration(
                          labelText: 'Message',
                          hintText:
                              'Ask OpenPinch something. This posts a local desktop message into the runtime.',
                        ),
                      ),
                      const SizedBox(height: 12),
                      Align(
                        alignment: Alignment.centerRight,
                        child: ElevatedButton.icon(
                          onPressed: controller.refreshing
                              ? null
                              : () async {
                                  final body = _messageController.text.trim();
                                  if (body.isEmpty) {
                                    return;
                                  }
                                  await controller.sendDesktopMessage(
                                    sender:
                                        _senderController.text.trim().isEmpty
                                            ? 'desktop-user'
                                            : _senderController.text.trim(),
                                    channelId:
                                        _channelController.text.trim().isEmpty
                                            ? 'desktop'
                                            : _channelController.text.trim(),
                                    body: body,
                                  );
                                  _messageController.clear();
                                },
                          icon: const Icon(Icons.send_rounded),
                          label: const Text('Post Desktop Message'),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
