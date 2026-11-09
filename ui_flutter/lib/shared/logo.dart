import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:material_ui/material_ui.dart';

import 'icons.dart';

/// The Pyxee mark (assets/logo/logo.svg). Colours are the logo's own; they do not follow the theme.
class PxLogo extends StatelessWidget {
  const PxLogo({super.key, this.size = 24});

  final double size;

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: size,
    child: CustomPaint(painter: _LogoPainter()),
  );
}

const _d =
    'M8150 11529 c-32 -13 -68 -47 -86 -81 -17 -34 -21 -2276 -4 -2308 6 -10 54 -54 108 -97 899 -718 1353 -1957 1127 -3080 -161 -796 -775 -1648 -1408 -1951 -87 -42 -221 -92 -245 -92 -3 0 46 32 109 71 595 369 1010 960 1160 1649 247 1140 -277 2376 -1255 2959 l-106 63 0 921 c-1 942 -5 1193 -22 1214 -32 40 -412 128 -723 168 -1453 185 -3069 -471 -4074 -1655 -121 -142 -144 -171 -223 -276 -900 -1194 -1241 -2863 -852 -4179 279 -943 1078 -2001 2019 -2671 467 -334 1102 -632 1549 -730 34 -7 120 -26 192 -43 559 -128 1154 -145 1727 -50 2375 397 4220 2654 4143 5069 -61 1893 -1046 3647 -2766 4926 -256 191 -288 205 -370 173z';

// translate(0 1254) scale(.1 -.1) as a column-major matrix
final ui.Path _logo = parseSvgPath(_d).transform(
  Float64List.fromList([0.1, 0, 0, 0, 0, -0.1, 0, 0, 0, 0, 1, 0, 0, 1254, 0, 1]),
);

class _LogoPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    canvas.save();
    // viewBox="96 69 1085 1085"
    canvas.scale(size.width / 1085, size.height / 1085);
    canvas.translate(-96, -69);
    canvas.drawPath(_logo, Paint()..color = const Color(0xFFF00524));
    canvas.save();
    canvas.clipPath(_logo);
    canvas.drawRect(const Rect.fromLTWH(452, 780, 68, 440), Paint()..color = const Color(0xFF0F0E0F));
    canvas.restore();
    canvas.drawCircle(const Offset(626.5, 627.5), 272, Paint()..color = const Color(0xFF0F0E0F));
    canvas.drawCircle(const Offset(626, 627), 150.5, Paint()..color = const Color(0xFFF2EDE2));
    canvas.drawCircle(const Offset(626.5, 630.5), 42.5, Paint()..color = const Color(0xFF0F0E0F));
    canvas.restore();
  }

  @override
  bool shouldRepaint(_LogoPainter old) => false;
}
