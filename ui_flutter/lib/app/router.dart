import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../features/collection/collection_page.dart';
import '../features/collection/playlist_page.dart';
import '../features/discover/discover_page.dart';
import '../features/search/search_page.dart';
import 'places.dart';
import 'shell/app_shell.dart';

final routerProvider = Provider<GoRouter>((ref) {
  final router = GoRouter(
    initialLocation: Place.discover.path,
    routes: [
      ShellRoute(
        builder: (context, state, child) =>
            AppShell(path: state.uri.path, child: child),
        routes: [
          GoRoute(
            path: Place.discover.path,
            pageBuilder: (context, state) =>
                _page(state, const DiscoverPage()),
          ),
          GoRoute(
            path: Place.collection.path,
            pageBuilder: (context, state) =>
                _page(state, const CollectionPage()),
            routes: [
              GoRoute(
                path: 'playlist/:id',
                pageBuilder: (context, state) => _page(
                  state,
                  PlaylistPage(
                    id: int.tryParse(state.pathParameters['id'] ?? ''),
                  ),
                ),
              ),
            ],
          ),
          GoRoute(
            path: Place.search.path,
            pageBuilder: (context, state) => _page(
              state,
              SearchPage(query: state.uri.queryParameters['q'] ?? ''),
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
