import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../screens/dashboard/dashboard_screen.dart';
import '../screens/charts/charts_screen.dart';
import '../screens/positions/positions_screen.dart';
import '../screens/ai_insights/ai_insights_screen.dart';
import '../screens/settings/settings_screen.dart';
import '../screens/shell/app_shell.dart';

final routerProvider = Provider<GoRouter>((ref) {
  return GoRouter(
    initialLocation: '/dashboard',
    routes: [
      ShellRoute(
        builder: (context, state, child) => AppShell(child: child),
        routes: [
          GoRoute(
            path: '/dashboard',
            pageBuilder: (context, state) => const NoTransitionPage(
              child: DashboardScreen(),
            ),
          ),
          GoRoute(
            path: '/charts',
            pageBuilder: (context, state) => const NoTransitionPage(
              child: ChartsScreen(),
            ),
          ),
          GoRoute(
            path: '/positions',
            pageBuilder: (context, state) => const NoTransitionPage(
              child: PositionsScreen(),
            ),
          ),
          GoRoute(
            path: '/ai',
            pageBuilder: (context, state) => const NoTransitionPage(
              child: AiInsightsScreen(),
            ),
          ),
          GoRoute(
            path: '/settings',
            pageBuilder: (context, state) => const NoTransitionPage(
              child: SettingsScreen(),
            ),
          ),
        ],
      ),
    ],
  );
});
