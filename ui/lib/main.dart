import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

void main() {
  runApp(const OpenPinchApp());
}

class OpenPinchApp extends StatelessWidget {
  const OpenPinchApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'OpenPinch Control',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        fontFamily: 'Georgia',
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF005F73),
          brightness: Brightness.light,
        ),
        scaffoldBackgroundColor: const Color(0xFFF4EFE4),
      ),
      home: const ControlPage(),
    );
  }
}

class ControlPage extends StatefulWidget {
  const ControlPage({super.key});

  @override
  State<ControlPage> createState() => _ControlPageState();
}

class _ControlPageState extends State<ControlPage> {
  final TextEditingController _senderController = TextEditingController(
    text: 'web-user',
  );
  final TextEditingController _messageController = TextEditingController();
  final TextEditingController _channelController = TextEditingController();

  Timer? _refreshTimer;
  Map<String, dynamic> _status = <String, dynamic>{};
  List<dynamic> _sessions = <dynamic>[];
  List<dynamic> _pairings = <dynamic>[];
  List<dynamic> _findings = <dynamic>[];
  List<dynamic> _profiles = <dynamic>[];
  String _doctorStatus = 'unknown';
  bool _loading = true;
  String _sendState = '';
  String? _selectedSessionId;
  List<dynamic> _messages = <dynamic>[];

  @override
  void initState() {
    super.initState();
    _refresh();
    _refreshTimer = Timer.periodic(
      const Duration(seconds: 5),
      (_) => _refresh(),
    );
  }

  @override
  void dispose() {
    _refreshTimer?.cancel();
    _senderController.dispose();
    _messageController.dispose();
    _channelController.dispose();
    super.dispose();
  }

  Future<void> _refresh() async {
    try {
      final status = await _getJson('/api/status');
      final sessions = await _getJson('/api/sessions?limit=12');
      final pairings = await _getJson('/api/pairings?limit=12');
      final doctor = await _getJson('/api/doctor');
      final profiles = await _getJson('/api/model-profiles');
      final selected = _selectedSessionId;
      Map<String, dynamic>? detail;
      if (selected != null && selected.isNotEmpty) {
        detail = await _getJson('/api/sessions/$selected?limit=50');
      }

      if (!mounted) {
        return;
      }
      setState(() {
        _status = status;
        _sessions = (sessions['sessions'] as List<dynamic>? ?? <dynamic>[]);
        _pairings = (pairings['pairings'] as List<dynamic>? ?? <dynamic>[]);
        _doctorStatus = doctor['status'] as String? ?? 'unknown';
        _findings = doctor['findings'] as List<dynamic>? ?? <dynamic>[];
        _profiles = profiles['profiles'] as List<dynamic>? ?? <dynamic>[];
        _messages = detail?['messages'] as List<dynamic>? ?? _messages;
        _loading = false;
      });
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _loading = false;
        _sendState = 'Refresh failed: $error';
      });
    }
  }

  Future<void> _sendWebChat() async {
    final sender = _senderController.text.trim().isEmpty
        ? 'web-user'
        : _senderController.text.trim();
    final body = _messageController.text.trim();
    if (body.isEmpty) {
      return;
    }

    setState(() {
      _sendState = 'Sending...';
    });

    final channelID = _channelController.text.trim().isEmpty
        ? sender
        : _channelController.text.trim();
    final response = await _postJson('/api/webchat/message', <String, dynamic>{
      'sender': sender,
      'channel_id': channelID,
      'body': body,
      'metadata_json': jsonEncode(<String, dynamic>{'source': 'flutter-web'}),
    });

    setState(() {
      _messageController.clear();
      _sendState = response['delivery_state'] as String? ?? 'sent';
      _selectedSessionId = response['session_id'] as String?;
    });
    await _refresh();
  }

  Future<Map<String, dynamic>> _getJson(String path) async {
    final response = await http.get(Uri.base.resolve(path));
    if (response.statusCode >= 400) {
      throw Exception(response.body);
    }
    return jsonDecode(response.body) as Map<String, dynamic>;
  }

  Future<Map<String, dynamic>> _postJson(
    String path,
    Map<String, dynamic> payload,
  ) async {
    final response = await http.post(
      Uri.base.resolve(path),
      headers: <String, String>{'Content-Type': 'application/json'},
      body: jsonEncode(payload),
    );
    if (response.statusCode >= 400) {
      throw Exception(response.body);
    }
    return jsonDecode(response.body) as Map<String, dynamic>;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      body: Container(
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
            colors: <Color>[
              Color(0xFFF4EFE4),
              Color(0xFFE6E0D1),
              Color(0xFFD8E3E7),
            ],
          ),
        ),
        child: SafeArea(
          child: RefreshIndicator(
            onRefresh: _refresh,
            child: ListView(
              padding: const EdgeInsets.all(20),
              children: <Widget>[
                if (_loading) ...<Widget>[
                  const LinearProgressIndicator(),
                  const SizedBox(height: 16),
                ],
                Text(
                  'OpenPinch Control',
                  style: theme.textTheme.displaySmall?.copyWith(
                    fontWeight: FontWeight.w700,
                    letterSpacing: -1.2,
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  'Sessions, pairings, doctor findings, model profiles, and webchat over the new gateway surface.',
                  style: theme.textTheme.bodyLarge?.copyWith(
                    color: const Color(0xFF4A5565),
                  ),
                ),
                const SizedBox(height: 20),
                Wrap(
                  spacing: 16,
                  runSpacing: 16,
                  children: <Widget>[
                    _StatCard(
                      label: 'Gateway',
                      value: _status['status'] as String? ?? 'unknown',
                      accent: const Color(0xFF0A9396),
                    ),
                    _StatCard(
                      label: 'Doctor',
                      value: _doctorStatus,
                      accent: const Color(0xFFBB3E03),
                    ),
                    _StatCard(
                      label: 'Sessions',
                      value: '${_sessions.length}',
                      accent: const Color(0xFF005F73),
                    ),
                    _StatCard(
                      label: 'Pairings',
                      value: '${_pairings.length}',
                      accent: const Color(0xFF9B2226),
                    ),
                  ],
                ),
                const SizedBox(height: 20),
                _Panel(
                  title: 'WebChat',
                  subtitle:
                      'Send a local webchat message into the Rust engine and watch session state update.',
                  child: Column(
                    children: <Widget>[
                      TextField(
                        controller: _senderController,
                        decoration: const InputDecoration(labelText: 'Sender'),
                      ),
                      const SizedBox(height: 12),
                      TextField(
                        controller: _channelController,
                        decoration: const InputDecoration(
                          labelText: 'Channel ID (optional)',
                        ),
                      ),
                      const SizedBox(height: 12),
                      TextField(
                        controller: _messageController,
                        minLines: 3,
                        maxLines: 5,
                        decoration: const InputDecoration(labelText: 'Message'),
                      ),
                      const SizedBox(height: 12),
                      Row(
                        children: <Widget>[
                          FilledButton(
                            onPressed: _sendWebChat,
                            child: const Text('Send To OpenPinch'),
                          ),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Text(
                              _sendState,
                              style: theme.textTheme.bodyMedium?.copyWith(
                                color: const Color(0xFF4A5565),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 20),
                LayoutBuilder(
                  builder: (BuildContext context, BoxConstraints constraints) {
                    final wide = constraints.maxWidth > 900;
                    final children = <Widget>[
                      Expanded(
                        child: _Panel(
                          title: 'Sessions',
                          subtitle:
                              'Connector routing, reply mode, and active session previews.',
                          child: Column(
                            children: _sessions
                                .map(
                                  (dynamic session) => _SessionTile(
                                    session: session as Map<String, dynamic>,
                                    selected:
                                        _selectedSessionId == session['id'],
                                    onTap: () async {
                                      setState(() {
                                        _selectedSessionId =
                                            session['id'] as String?;
                                      });
                                      await _refresh();
                                    },
                                  ),
                                )
                                .toList(),
                          ),
                        ),
                      ),
                      Expanded(
                        child: _Panel(
                          title: 'Conversation',
                          subtitle: _selectedSessionId == null
                              ? 'Select a session to inspect recorded messages.'
                              : 'Session $_selectedSessionId',
                          child: Column(
                            children: _messages.isEmpty
                                ? <Widget>[
                                    const Text('No session selected yet.'),
                                  ]
                                : _messages
                                    .map(
                                      (dynamic entry) => _MessageBubble(
                                        entry: entry as Map<String, dynamic>,
                                      ),
                                    )
                                    .toList(),
                          ),
                        ),
                      ),
                    ];
                    if (wide) {
                      return Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          children[0],
                          const SizedBox(width: 16),
                          children[1],
                        ],
                      );
                    }
                    return Column(
                      children: <Widget>[
                        children[0],
                        const SizedBox(height: 16),
                        children[1],
                      ],
                    );
                  },
                ),
                const SizedBox(height: 20),
                LayoutBuilder(
                  builder: (BuildContext context, BoxConstraints constraints) {
                    final wide = constraints.maxWidth > 900;
                    final left = Expanded(
                      child: _Panel(
                        title: 'Pairings',
                        subtitle: 'Approval state for direct-message sessions.',
                        child: Column(
                          children: _pairings
                              .map(
                                (dynamic entry) => _SimpleRow(
                                  title:
                                      "${entry['connector']} • ${entry['sender']}",
                                  subtitle:
                                      "${entry['status']} • ${entry['reason']}",
                                ),
                              )
                              .toList(),
                        ),
                      ),
                    );
                    final right = Expanded(
                      child: _Panel(
                        title: 'Doctor + Profiles',
                        subtitle: 'Runtime health and model routing defaults.',
                        child: Column(
                          children: <Widget>[
                            ..._findings.map(
                              (dynamic entry) => _SimpleRow(
                                title:
                                    "${entry['component']} • ${entry['status']}",
                                subtitle: entry['summary'] as String? ?? '',
                              ),
                            ),
                            const Divider(height: 28),
                            ..._profiles.map(
                              (dynamic profile) => _SimpleRow(
                                title:
                                    "${profile['name']} • ${profile['mode']}",
                                subtitle:
                                    (profile['provider_order'] as List<dynamic>)
                                        .join(', '),
                              ),
                            ),
                          ],
                        ),
                      ),
                    );
                    if (wide) {
                      return Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          left,
                          const SizedBox(width: 16),
                          right,
                        ],
                      );
                    }
                    return Column(
                      children: <Widget>[
                        left,
                        const SizedBox(height: 16),
                        right,
                      ],
                    );
                  },
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _StatCard extends StatelessWidget {
  const _StatCard({
    required this.label,
    required this.value,
    required this.accent,
  });

  final String label;
  final String value;
  final Color accent;

  @override
  Widget build(BuildContext context) {
    return AnimatedContainer(
      duration: const Duration(milliseconds: 240),
      width: 200,
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.82),
        borderRadius: BorderRadius.circular(24),
        boxShadow: const <BoxShadow>[
          BoxShadow(
            color: Color(0x14000000),
            blurRadius: 28,
            offset: Offset(0, 14),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Container(
            width: 12,
            height: 12,
            decoration: BoxDecoration(color: accent, shape: BoxShape.circle),
          ),
          const SizedBox(height: 16),
          Text(label, style: Theme.of(context).textTheme.labelLarge),
          const SizedBox(height: 8),
          Text(value, style: Theme.of(context).textTheme.headlineSmall),
        ],
      ),
    );
  }
}

class _Panel extends StatelessWidget {
  const _Panel({
    required this.title,
    required this.subtitle,
    required this.child,
  });

  final String title;
  final String subtitle;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.84),
        borderRadius: BorderRadius.circular(28),
        border: Border.all(color: const Color(0x14000000)),
        boxShadow: const <BoxShadow>[
          BoxShadow(
            color: Color(0x14000000),
            blurRadius: 28,
            offset: Offset(0, 16),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(title, style: Theme.of(context).textTheme.headlineSmall),
          const SizedBox(height: 8),
          Text(subtitle, style: Theme.of(context).textTheme.bodyMedium),
          const SizedBox(height: 18),
          child,
        ],
      ),
    );
  }
}

class _SessionTile extends StatelessWidget {
  const _SessionTile({
    required this.session,
    required this.selected,
    required this.onTap,
  });

  final Map<String, dynamic> session;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final title = "${session['title']} • ${session['connector']}";
    final preview = session['last_message_preview'] as String? ?? '';
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(18),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 180),
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            color: selected ? const Color(0xFFD6EBEF) : const Color(0xFFF8F5EF),
            borderRadius: BorderRadius.circular(18),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(title),
              const SizedBox(height: 6),
              Text(preview, style: Theme.of(context).textTheme.bodySmall),
            ],
          ),
        ),
      ),
    );
  }
}

class _MessageBubble extends StatelessWidget {
  const _MessageBubble({required this.entry});

  final Map<String, dynamic> entry;

  @override
  Widget build(BuildContext context) {
    final isAssistant = entry['role'] == 'assistant';
    final heading = "${entry['sender']} • ${entry['role']}";
    final body = entry['body'] as String? ?? '';
    return Align(
      alignment: isAssistant ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.only(bottom: 10),
        padding: const EdgeInsets.all(14),
        constraints: const BoxConstraints(maxWidth: 420),
        decoration: BoxDecoration(
          color:
              isAssistant ? const Color(0xFF005F73) : const Color(0xFFEDE4D4),
          borderRadius: BorderRadius.circular(18),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              heading,
              style: TextStyle(
                color: isAssistant ? Colors.white70 : const Color(0xFF475467),
                fontSize: 12,
              ),
            ),
            const SizedBox(height: 6),
            Text(
              body,
              style: TextStyle(
                color: isAssistant ? Colors.white : const Color(0xFF182024),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SimpleRow extends StatelessWidget {
  const _SimpleRow({required this.title, required this.subtitle});

  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(title, style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 4),
          Text(subtitle, style: Theme.of(context).textTheme.bodySmall),
        ],
      ),
    );
  }
}
