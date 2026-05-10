import 'package:flutter/material.dart';

import '../shared/theme.dart';
import 'router.dart';

class OpenPinchDesktopApp extends StatefulWidget {
  const OpenPinchDesktopApp({super.key});

  @override
  State<OpenPinchDesktopApp> createState() => _OpenPinchDesktopAppState();
}

class _OpenPinchDesktopAppState extends State<OpenPinchDesktopApp> {
  late final _router = buildRouter();

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'OpenPinch Desktop',
      debugShowCheckedModeBanner: false,
      theme: openPinchTheme(),
      routerConfig: _router,
    );
  }
}
