String? normalizePdfOutputName(String value) {
  final name = value.trim();
  if (name.isEmpty ||
      name == '.' ||
      name == '..' ||
      name.endsWith(' ') ||
      name.endsWith('.') ||
      name.contains(RegExp(r'[<>:"/\\|?*\x00-\x1F]'))) {
    return null;
  }
  final normalized = name.toLowerCase().endsWith('.pdf') ? name : '$name.pdf';
  final dot = normalized.lastIndexOf('.');
  final stem = dot <= 0 ? '' : normalized.substring(0, dot);
  final basename = stem.split('.').first.toUpperCase();
  const reserved = {
    'CON',
    'PRN',
    'AUX',
    'NUL',
    'COM1',
    'COM2',
    'COM3',
    'COM4',
    'COM5',
    'COM6',
    'COM7',
    'COM8',
    'COM9',
    'LPT1',
    'LPT2',
    'LPT3',
    'LPT4',
    'LPT5',
    'LPT6',
    'LPT7',
    'LPT8',
    'LPT9',
  };
  return stem.isEmpty || reserved.contains(basename) ? null : normalized;
}
