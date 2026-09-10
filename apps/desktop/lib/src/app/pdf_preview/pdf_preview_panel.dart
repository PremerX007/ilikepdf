import 'dart:io';

import 'package:flutter/material.dart';
import 'package:ilikepdf/src/app/pdf_preview/pdf_preview_workflow.dart';

class PdfPreviewPanel extends StatefulWidget {
  const PdfPreviewPanel({required this.workflow, super.key});

  final PdfPreviewWorkflow workflow;

  @override
  State<PdfPreviewPanel> createState() => _PdfPreviewPanelState();
}

class _PdfPreviewPanelState extends State<PdfPreviewPanel> {
  SelectedPdf? _selectedPdf;
  RenderedPdfPage? _renderedPage;
  String? _errorMessage;
  bool _isBusy = false;

  Future<void> _selectPdf() async {
    setState(() {
      _isBusy = true;
      _errorMessage = null;
    });

    try {
      final selected = await widget.workflow.selectAndInspect();
      if (!mounted || selected == null) {
        return;
      }
      setState(() {
        _selectedPdf = selected;
        _renderedPage = null;
      });
    } on Object {
      if (mounted) {
        setState(() {
          _errorMessage = 'This PDF could not be opened locally.';
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _isBusy = false;
        });
      }
    }
  }

  Future<void> _renderFirstPage() async {
    final selected = _selectedPdf;
    if (selected == null) {
      return;
    }

    setState(() {
      _isBusy = true;
      _errorMessage = null;
    });
    try {
      final rendered = await widget.workflow.renderFirstPage(
        selected.sourcePath,
      );
      if (mounted) {
        setState(() {
          _renderedPage = rendered;
        });
      }
    } on Object {
      if (mounted) {
        setState(() {
          _errorMessage = 'Page 1 could not be rendered locally.';
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _isBusy = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final selected = _selectedPdf;
    final rendered = _renderedPage;

    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 900),
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(32),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                'Local PDF preview',
                style: Theme.of(context).textTheme.headlineMedium,
              ),
              const SizedBox(height: 8),
              const Text(
                'Choose a PDF from this computer. The document is processed locally and is never uploaded.',
              ),
              const SizedBox(height: 24),
              Align(
                alignment: Alignment.centerLeft,
                child: FilledButton.icon(
                  onPressed: _isBusy ? null : _selectPdf,
                  icon: const Icon(Icons.file_open_outlined),
                  label: const Text('Select PDF'),
                ),
              ),
              if (_isBusy) ...[
                const SizedBox(height: 16),
                const LinearProgressIndicator(),
              ],
              if (_errorMessage case final error?) ...[
                const SizedBox(height: 16),
                Text(
                  error,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ],
              if (selected != null) ...[
                const SizedBox(height: 24),
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(20),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          selected.displayName,
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                        const SizedBox(height: 8),
                        Text('${selected.pageCount} pages'),
                        if (selected.firstPageWidthPoints case final width?)
                          Text(
                            'First page: ${width.toStringAsFixed(0)} × ${selected.firstPageHeightPoints?.toStringAsFixed(0)} points',
                          ),
                        const SizedBox(height: 16),
                        FilledButton.tonalIcon(
                          onPressed: _isBusy || selected.pageCount == 0
                              ? null
                              : _renderFirstPage,
                          icon: const Icon(Icons.image_outlined),
                          label: const Text('Render page 1'),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
              if (rendered != null) ...[
                const SizedBox(height: 24),
                Text(
                  'Rendered preview',
                  style: Theme.of(context).textTheme.titleLarge,
                ),
                const SizedBox(height: 8),
                Text(
                  '${rendered.widthPixels} × ${rendered.heightPixels} pixels',
                ),
                const SizedBox(height: 12),
                DecoratedBox(
                  decoration: BoxDecoration(
                    border: Border.all(
                      color: Theme.of(context).colorScheme.outlineVariant,
                    ),
                  ),
                  child: Image.file(
                    File(rendered.outputPath),
                    fit: BoxFit.contain,
                    errorBuilder: (context, error, stackTrace) => const Padding(
                      padding: EdgeInsets.all(24),
                      child: Text('The rendered PNG could not be displayed.'),
                    ),
                  ),
                ),
                const SizedBox(height: 8),
                SelectableText('Output: ${rendered.outputPath}'),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
