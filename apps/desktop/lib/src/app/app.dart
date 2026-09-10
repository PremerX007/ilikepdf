import 'package:flutter/material.dart';

class IlikepdfApp extends StatelessWidget {
  const IlikepdfApp({
    required this.applicationName,
    required this.coreVersion,
    required this.localOnly,
    super.key,
  });

  final String applicationName;
  final String coreVersion;
  final bool localOnly;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: applicationName,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFFB42318)),
        useMaterial3: true,
      ),
      home: Scaffold(
        appBar: AppBar(title: Text(applicationName)),
        body: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 520),
            child: Padding(
              padding: const EdgeInsets.all(32),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Icon(Icons.picture_as_pdf_outlined, size: 72),
                  const SizedBox(height: 24),
                  Text(
                    'Local PDF tools are being prepared.',
                    style: Theme.of(context).textTheme.headlineSmall,
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 12),
                  Text(
                    localOnly
                        ? 'Privacy mode: local processing only'
                        : 'Privacy mode unavailable',
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 8),
                  Text('Rust core $coreVersion'),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
