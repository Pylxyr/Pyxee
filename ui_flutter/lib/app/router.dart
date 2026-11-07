import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../features/explore/explore_page.dart';
import '../features/home/home_page.dart';
import '../features/library/library_page.dart';
import '../features/playlist/playlist_page.dart';
import '../features/queue/queue_page.dart';
import '../features/search/search_page.dart';
import 'shell/app_shell.dart';

final routerProvider = Provider<GoRouter>((ref) {
  final router = GoRouter(
    initialLocation: '/home',
    routes: [
      ShellRoute(
        builder: (context, state, child) =>
            AppShell(path: state.uri.path, child: child),
        routes: [
          GoRoute(
            path: '/home',
            pageBuilder: (context, state) => _page(state, const HomePage()),
          ),
          GoRoute(
            path: '/explore',
            pageBuilder: (context, state) => _page(state, const ExplorePage()),
          ),
          GoRoute(
            path: '/library',
            pageBuilder: (context, state) => _page(state, const LibraryPage()),
          ),
          GoRoute(
            path: '/queue',
            pageBuilder: (context, state) => _page(state, const QueuePage()),
          ),
          GoRoute(
            path: '/search',
            pageBuilder: (context, state) => _page(
              state,
              SearchPage(query: state.uri.queryParameters['q'] ?? ''),
            ),
          ),
          GoRoute(
            path: '/playlist/:id',
            pageBuilder: (context, state) => _page(
              state,
              PlaylistPage(id: int.tryParse(state.pathParameters['id'] ?? '')),
            ),
          ),
        ],
      ),
    ],
  );
  ref.onDispose(router.dispose);
  return router;
});

Page<void> _page(GoRouterState state, Widget child) =>
    NoTransitionPage<void>(key: state.pageKey, child: child);
