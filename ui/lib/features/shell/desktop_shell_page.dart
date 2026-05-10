import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../core/state/openpinch_controller.dart';
import '../../shared/theme.dart';

class DesktopShellPage extends ConsumerStatefulWidget {
  const DesktopShellPage({
    required this.currentLocation,
    required this.child,
    super.key,
  });

  final String currentLocation;
  final Widget child;

  @override
  ConsumerState<DesktopShellPage> createState() => _DesktopShellPageState();
}

class _DesktopShellPageState extends ConsumerState<DesktopShellPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(openPinchControllerProvider).bootstrap();
    });
  }

  @override
  Widget build(BuildContext context) {
    final controller = ref.watch(openPinchControllerProvider);
    final palette = context.palette;
    final theme = Theme.of(context);
    final navItems = _navItems;
    final index = navItems.indexWhere(
      (item) => widget.currentLocation.startsWith(item.route),
    );

    return CallbackShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.keyK, control: true): () =>
            _showCommandPalette(context, controller),
        const SingleActivator(LogicalKeyboardKey.keyK, meta: true): () =>
            _showCommandPalette(context, controller),
        const SingleActivator(LogicalKeyboardKey.keyR, control: true):
            controller.refresh,
        const SingleActivator(LogicalKeyboardKey.keyR, meta: true):
            controller.refresh,
      },
      child: Scaffold(
        body: Container(
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: <Color>[
                palette.sand,
                palette.sandDark,
                palette.mist.withValues(alpha: 0.72),
              ],
            ),
          ),
          child: SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                children: <Widget>[
                  _HeaderBar(
                    controller: controller,
                    onRefresh: controller.refresh,
                    onPalette: () => _showCommandPalette(context, controller),
                  ),
                  const SizedBox(height: 18),
                  Expanded(
                    child: Row(
                      children: <Widget>[
                        Card(
                          child: Padding(
                            padding: const EdgeInsets.all(12),
                            child: NavigationRail(
                              extended: MediaQuery.sizeOf(context).width > 1320,
                              minExtendedWidth: 212,
                              selectedIndex: index < 0 ? 0 : index,
                              onDestinationSelected: (selected) {
                                context.go(navItems[selected].route);
                              },
                              labelType: MediaQuery.sizeOf(context).width > 1320
                                  ? NavigationRailLabelType.none
                                  : NavigationRailLabelType.all,
                              leading: Container(
                                width: 52,
                                height: 52,
                                decoration: BoxDecoration(
                                  color: palette.teal,
                                  borderRadius: BorderRadius.circular(18),
                                ),
                                child: const Icon(
                                  Icons.hub_rounded,
                                  color: Colors.white,
                                ),
                              ),
                              trailing: Expanded(
                                child: Align(
                                  alignment: Alignment.bottomCenter,
                                  child: FilledButton.tonalIcon(
                                    onPressed: controller.shutdownHost,
                                    icon: const Icon(Icons.power_settings_new),
                                    label: const Text('Stop Host'),
                                  ),
                                ),
                              ),
                              destinations: navItems
                                  .map(
                                    (item) => NavigationRailDestination(
                                      icon: Icon(item.icon),
                                      selectedIcon: Icon(item.icon),
                                      label: Text(item.label),
                                    ),
                                  )
                                  .toList(growable: false),
                            ),
                          ),
                        ),
                        const SizedBox(width: 18),
                        Expanded(
                          child: Card(
                            clipBehavior: Clip.antiAlias,
                            child: Container(
                              decoration: BoxDecoration(
                                gradient: LinearGradient(
                                  begin: Alignment.topCenter,
                                  end: Alignment.bottomCenter,
                                  colors: <Color>[
                                    Colors.white.withValues(alpha: 0.95),
                                    palette.sand.withValues(alpha: 0.45),
                                  ],
                                ),
                              ),
                              child: SelectionArea(
                                child: Padding(
                                  padding: const EdgeInsets.all(20),
                                  child: DefaultTextStyle(
                                    style: theme.textTheme.bodyMedium!,
                                    child: widget.child,
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _showCommandPalette(
    BuildContext context,
    OpenPinchController controller,
  ) async {
    final commands = <_PaletteCommand>[
      ..._navItems.map(
        (item) => _PaletteCommand(
          title: item.label,
          subtitle: 'Open ${item.label.toLowerCase()}',
          icon: item.icon,
          onSelected: () => context.go(item.route),
        ),
      ),
      _PaletteCommand(
        title: 'Refresh Runtime',
        subtitle: 'Reload status, sessions, pairings, and brain state',
        icon: Icons.refresh_rounded,
        onSelected: controller.refresh,
      ),
      _PaletteCommand(
        title: 'Restart Desktop Host',
        subtitle: 'Ensure the bundled runtime is active',
        icon: Icons.rocket_launch_rounded,
        onSelected: controller.restartHost,
      ),
      _PaletteCommand(
        title: 'Shutdown Desktop Host',
        subtitle: 'Stop the bundled runtime sidecars',
        icon: Icons.power_settings_new_rounded,
        onSelected: controller.shutdownHost,
      ),
    ];

    await showDialog<void>(
      context: context,
      builder: (context) {
        return Dialog(
          insetPadding:
              const EdgeInsets.symmetric(horizontal: 120, vertical: 72),
          child: SizedBox(
            width: 720,
            child: ListView.separated(
              padding: const EdgeInsets.all(18),
              itemCount: commands.length + 1,
              separatorBuilder: (_, __) => const SizedBox(height: 8),
              itemBuilder: (context, index) {
                if (index == 0) {
                  return const Padding(
                    padding: EdgeInsets.only(bottom: 8),
                    child: TextField(
                      enabled: false,
                      decoration: InputDecoration(
                        prefixIcon: Icon(Icons.search_rounded),
                        hintText:
                            'Command palette is keyboard-first in this build. Ctrl/Cmd+R refreshes.',
                      ),
                    ),
                  );
                }
                final command = commands[index - 1];
                return ListTile(
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(18),
                  ),
                  tileColor: Theme.of(context)
                      .colorScheme
                      .secondary
                      .withValues(alpha: 0.08),
                  leading: Icon(command.icon),
                  title: Text(command.title),
                  subtitle: Text(command.subtitle),
                  onTap: () {
                    Navigator.of(context).pop();
                    command.onSelected();
                  },
                );
              },
            ),
          ),
        );
      },
    );
  }
}

class _HeaderBar extends StatelessWidget {
  const _HeaderBar({
    required this.controller,
    required this.onRefresh,
    required this.onPalette,
  });

  final OpenPinchController controller;
  final Future<void> Function() onRefresh;
  final VoidCallback onPalette;

  @override
  Widget build(BuildContext context) {
    final palette = context.palette;
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 20),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(30),
        gradient: LinearGradient(
          colors: <Color>[
            palette.teal,
            palette.aqua,
            palette.ember.withValues(alpha: 0.88),
          ],
        ),
      ),
      child: Row(
        children: <Widget>[
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  'OpenPinch Desktop',
                  style: theme.textTheme.displaySmall?.copyWith(
                    color: Colors.white,
                    fontSize: 36,
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  'Native control shell for sessions, pairings, brain memory, models, and bundled runtime health.',
                  style: theme.textTheme.bodyLarge?.copyWith(
                    color: Colors.white.withValues(alpha: 0.9),
                  ),
                ),
              ],
            ),
          ),
          Wrap(
            spacing: 10,
            runSpacing: 10,
            children: <Widget>[
              _StatusChip(
                icon: Icons.memory_rounded,
                label: controller.host['status']?.toString() ?? 'host',
              ),
              _StatusChip(
                icon: Icons.network_check_rounded,
                label: controller.status['status']?.toString() ?? 'runtime',
              ),
              FilledButton.tonalIcon(
                onPressed: onPalette,
                icon: const Icon(Icons.keyboard_command_key_rounded),
                label: const Text('Palette'),
              ),
              ElevatedButton.icon(
                onPressed: controller.refreshing ? null : onRefresh,
                icon: const Icon(Icons.refresh_rounded),
                label: Text(controller.refreshing ? 'Refreshing' : 'Refresh'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.icon, required this.label});

  final IconData icon;
  final String label;

  @override
  Widget build(BuildContext context) {
    return Chip(
      avatar: Icon(icon, size: 18),
      label: Text(label),
      backgroundColor: Colors.white.withValues(alpha: 0.16),
      labelStyle: const TextStyle(
        color: Colors.white,
        fontWeight: FontWeight.w700,
      ),
      side: BorderSide.none,
    );
  }
}

class _NavItem {
  const _NavItem(this.label, this.route, this.icon);

  final String label;
  final String route;
  final IconData icon;
}

class _PaletteCommand {
  const _PaletteCommand({
    required this.title,
    required this.subtitle,
    required this.icon,
    required this.onSelected,
  });

  final String title;
  final String subtitle;
  final IconData icon;
  final VoidCallback onSelected;
}

const _navItems = <_NavItem>[
  _NavItem('Dashboard', '/dashboard', Icons.space_dashboard_rounded),
  _NavItem('Sessions', '/sessions', Icons.forum_rounded),
  _NavItem('Brain', '/brain', Icons.psychology_rounded),
  _NavItem('Operations', '/operations', Icons.hub_outlined),
  _NavItem('Settings', '/settings', Icons.tune_rounded),
];
