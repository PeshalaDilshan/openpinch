import 'package:go_router/go_router.dart';

import '../features/brain/brain_page.dart';
import '../features/dashboard/dashboard_page.dart';
import '../features/operations/operations_page.dart';
import '../features/settings/settings_page.dart';
import '../features/sessions/sessions_page.dart';
import '../features/shell/desktop_shell_page.dart';

GoRouter buildRouter() {
  return GoRouter(
    initialLocation: '/dashboard',
    routes: <RouteBase>[
      GoRoute(
        path: '/dashboard',
        pageBuilder: (context, state) => NoTransitionPage<void>(
          child: DesktopShellPage(
            currentLocation: state.uri.path,
            child: const DashboardPage(),
          ),
        ),
      ),
      GoRoute(
        path: '/sessions',
        pageBuilder: (context, state) => NoTransitionPage<void>(
          child: DesktopShellPage(
            currentLocation: state.uri.path,
            child: const SessionsPage(),
          ),
        ),
      ),
      GoRoute(
        path: '/brain',
        pageBuilder: (context, state) => NoTransitionPage<void>(
          child: DesktopShellPage(
            currentLocation: state.uri.path,
            child: const BrainPage(),
          ),
        ),
      ),
      GoRoute(
        path: '/operations',
        pageBuilder: (context, state) => NoTransitionPage<void>(
          child: DesktopShellPage(
            currentLocation: state.uri.path,
            child: const OperationsPage(),
          ),
        ),
      ),
      GoRoute(
        path: '/settings',
        pageBuilder: (context, state) => NoTransitionPage<void>(
          child: DesktopShellPage(
            currentLocation: state.uri.path,
            child: const SettingsPage(),
          ),
        ),
      ),
    ],
  );
}
