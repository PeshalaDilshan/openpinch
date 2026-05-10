import 'package:flutter/material.dart';

ThemeData openPinchTheme() {
  const sand = Color(0xFFF4EFE4);
  const sandDark = Color(0xFFE6E0D1);
  const mist = Color(0xFFD8E3E7);
  const ink = Color(0xFF16313A);
  const teal = Color(0xFF005F73);
  const aqua = Color(0xFF0A9396);
  const ember = Color(0xFFBB3E03);
  const wine = Color(0xFF9B2226);

  final colorScheme = ColorScheme(
    brightness: Brightness.light,
    primary: teal,
    onPrimary: Colors.white,
    secondary: aqua,
    onSecondary: Colors.white,
    error: wine,
    onError: Colors.white,
    surface: Colors.white.withValues(alpha: 0.92),
    onSurface: ink,
  );

  return ThemeData(
    useMaterial3: true,
    colorScheme: colorScheme,
    fontFamily: 'Georgia',
    scaffoldBackgroundColor: sand,
    textTheme: const TextTheme(
      displaySmall: TextStyle(
        fontSize: 34,
        fontWeight: FontWeight.w700,
        color: ink,
        letterSpacing: -1.2,
      ),
      headlineSmall: TextStyle(
        fontSize: 24,
        fontWeight: FontWeight.w700,
        color: ink,
      ),
      titleLarge: TextStyle(
        fontSize: 18,
        fontWeight: FontWeight.w700,
        color: ink,
      ),
      bodyLarge: TextStyle(
        fontSize: 15,
        height: 1.45,
        color: ink,
      ),
      bodyMedium: TextStyle(
        fontSize: 13,
        height: 1.4,
        color: Color(0xFF4A5565),
      ),
    ),
    cardTheme: CardThemeData(
      elevation: 0,
      color: Colors.white.withValues(alpha: 0.9),
      shadowColor: Colors.transparent,
      margin: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(24),
        side: BorderSide(color: mist.withValues(alpha: 0.8)),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: Colors.white.withValues(alpha: 0.92),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(18),
        borderSide: const BorderSide(color: mist),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(18),
        borderSide: const BorderSide(color: mist),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(18),
        borderSide: const BorderSide(color: aqua, width: 1.5),
      ),
    ),
    chipTheme: ChipThemeData(
      backgroundColor: mist.withValues(alpha: 0.6),
      selectedColor: aqua.withValues(alpha: 0.16),
      labelStyle: const TextStyle(color: ink, fontWeight: FontWeight.w600),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(999)),
      side: BorderSide.none,
    ),
    navigationRailTheme: const NavigationRailThemeData(
      backgroundColor: Colors.transparent,
      selectedIconTheme: IconThemeData(color: aqua),
      selectedLabelTextStyle: TextStyle(
        color: ink,
        fontWeight: FontWeight.w700,
      ),
      unselectedLabelTextStyle: TextStyle(color: Color(0xFF4A5565)),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: teal,
        foregroundColor: Colors.white,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
        padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
      ).copyWith(
        overlayColor: WidgetStatePropertyAll(aqua.withValues(alpha: 0.16)),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: ink,
        side: BorderSide(color: teal.withValues(alpha: 0.24)),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
        padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
      ),
    ),
    dividerColor: sandDark,
    extensions: const <ThemeExtension<dynamic>>[
      OpenPinchPalette(
        sand: sand,
        sandDark: sandDark,
        mist: mist,
        ink: ink,
        teal: teal,
        aqua: aqua,
        ember: ember,
        wine: wine,
      ),
    ],
  );
}

@immutable
class OpenPinchPalette extends ThemeExtension<OpenPinchPalette> {
  const OpenPinchPalette({
    required this.sand,
    required this.sandDark,
    required this.mist,
    required this.ink,
    required this.teal,
    required this.aqua,
    required this.ember,
    required this.wine,
  });

  final Color sand;
  final Color sandDark;
  final Color mist;
  final Color ink;
  final Color teal;
  final Color aqua;
  final Color ember;
  final Color wine;

  @override
  OpenPinchPalette copyWith({
    Color? sand,
    Color? sandDark,
    Color? mist,
    Color? ink,
    Color? teal,
    Color? aqua,
    Color? ember,
    Color? wine,
  }) {
    return OpenPinchPalette(
      sand: sand ?? this.sand,
      sandDark: sandDark ?? this.sandDark,
      mist: mist ?? this.mist,
      ink: ink ?? this.ink,
      teal: teal ?? this.teal,
      aqua: aqua ?? this.aqua,
      ember: ember ?? this.ember,
      wine: wine ?? this.wine,
    );
  }

  @override
  ThemeExtension<OpenPinchPalette> lerp(
    covariant ThemeExtension<OpenPinchPalette>? other,
    double t,
  ) {
    if (other is! OpenPinchPalette) {
      return this;
    }
    return OpenPinchPalette(
      sand: Color.lerp(sand, other.sand, t) ?? sand,
      sandDark: Color.lerp(sandDark, other.sandDark, t) ?? sandDark,
      mist: Color.lerp(mist, other.mist, t) ?? mist,
      ink: Color.lerp(ink, other.ink, t) ?? ink,
      teal: Color.lerp(teal, other.teal, t) ?? teal,
      aqua: Color.lerp(aqua, other.aqua, t) ?? aqua,
      ember: Color.lerp(ember, other.ember, t) ?? ember,
      wine: Color.lerp(wine, other.wine, t) ?? wine,
    );
  }
}

extension OpenPinchPaletteContext on BuildContext {
  OpenPinchPalette get palette => Theme.of(this).extension<OpenPinchPalette>()!;
}
