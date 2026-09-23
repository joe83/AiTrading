import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/theme.dart';

class AiInsightsScreen extends ConsumerWidget {
  const AiInsightsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return SafeArea(
      child: CustomScrollView(
        physics: const BouncingScrollPhysics(),
        slivers: [
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
              child: Text('AI Insights', style: Theme.of(context).textTheme.displayMedium),
            ),
          ),

          // AI Status Card
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
              child: _buildAiStatusCard(context),
            ),
          ),

          // Market Sentiment
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 20, 20, 8),
              child: Text('Market Sentiment', style: Theme.of(context).textTheme.titleLarge),
            ),
          ),

          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20),
              child: Row(
                children: [
                  Expanded(child: _buildSentimentGauge(context, 'Crypto', 0.72, AppTheme.primaryGreen, 'Bullish')),
                  const SizedBox(width: 12),
                  Expanded(child: _buildSentimentGauge(context, 'Overall', 0.45, AppTheme.accentBlue, 'Neutral')),
                ],
              ),
            ),
          ),

          // Latest Analyses
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 24, 20, 8),
              child: Text('Latest Analyses', style: Theme.of(context).textTheme.titleLarge),
            ),
          ),

          SliverList(
            delegate: SliverChildListDelegate([
              _buildAnalysisCard(
                context,
                symbol: 'BTCUSDT',
                trend: 'Bullish',
                confidence: 0.82,
                analysis: 'MACD bullish crossover confirmed on 1H. RSI at 62 (room to run). EMA(9) above EMA(21). Volume increasing 1.4x above 20-SMA. Three White Soldiers pattern detected.',
                indicators: {'RSI': '62.1', 'MACD': 'Bullish', 'EMA': 'Bullish Cross', 'BB %B': '0.78'},
                action: 'BUY',
              ),
              _buildAnalysisCard(
                context,
                symbol: 'ETHUSDT',
                trend: 'Bullish',
                confidence: 0.71,
                analysis: 'Following BTC momentum. RSI neutral at 55. Approaching key resistance at \$3,500. Bollinger squeeze forming — potential breakout incoming.',
                indicators: {'RSI': '55.3', 'MACD': 'Neutral', 'EMA': 'Bullish', 'ATR': '45.2'},
                action: 'BUY',
              ),
              _buildAnalysisCard(
                context,
                symbol: 'SOLUSDT',
                trend: 'Bearish',
                confidence: 0.65,
                analysis: 'Bearish divergence on RSI (price up, RSI down). MACD histogram weakening. Near strong resistance at \$145. Recommend short or take profit on longs.',
                indicators: {'RSI': '68.5', 'MACD': 'Bearish Div', 'EMA': 'Bullish', 'Stoch': '82.1'},
                action: 'SELL',
              ),
            ]),
          ),

          // Grok API Usage
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 24, 20, 8),
              child: Text('Grok API Usage', style: Theme.of(context).textTheme.titleLarge),
            ),
          ),
          SliverToBoxAdapter(
            child: Container(
              margin: const EdgeInsets.symmetric(horizontal: 20),
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(
                color: AppTheme.bgCard,
                borderRadius: BorderRadius.circular(14),
                border: Border.all(color: const Color(0xFF1E293B)),
              ),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceAround,
                children: [
                  _usageMetric(context, 'Tokens Today', '12,450'),
                  _usageMetric(context, 'API Calls', '24'),
                  _usageMetric(context, 'Est. Cost', '\$0.18'),
                ],
              ),
            ),
          ),

          const SliverToBoxAdapter(child: SizedBox(height: 100)),
        ],
      ),
    );
  }

  Widget _buildAiStatusCard(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        gradient: const LinearGradient(
          colors: [Color(0xFF1A1A4E), Color(0xFF111827)],
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
        ),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppTheme.accentPurple.withValues(alpha: 0.3)),
      ),
      child: Row(
        children: [
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: AppTheme.accentPurple.withValues(alpha: 0.2),
              borderRadius: BorderRadius.circular(12),
            ),
            child: const Icon(Icons.psychology_rounded, color: AppTheme.accentPurple, size: 28),
          ),
          const SizedBox(width: 16),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text('Grok AI Engine', style: TextStyle(fontWeight: FontWeight.w600, fontSize: 16, color: AppTheme.textPrimary)),
                const SizedBox(height: 4),
                Row(
                  children: [
                    Container(width: 6, height: 6, decoration: const BoxDecoration(color: AppTheme.primaryGreen, shape: BoxShape.circle)),
                    const SizedBox(width: 6),
                    const Text('Active · xAI Grok', style: TextStyle(color: AppTheme.textSecondary, fontSize: 13)),
                  ],
                ),
                const SizedBox(height: 4),
                const Text('Last analysis: 30s ago', style: TextStyle(color: AppTheme.textMuted, fontSize: 12)),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildSentimentGauge(BuildContext context, String label, double value, Color color, String text) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.bgCard,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: const Color(0xFF1E293B)),
      ),
      child: Column(
        children: [
          Text(label, style: const TextStyle(color: AppTheme.textSecondary, fontSize: 13)),
          const SizedBox(height: 12),
          Stack(
            alignment: Alignment.center,
            children: [
              SizedBox(
                width: 64,
                height: 64,
                child: CircularProgressIndicator(
                  value: value,
                  strokeWidth: 6,
                  backgroundColor: const Color(0xFF1E293B),
                  valueColor: AlwaysStoppedAnimation<Color>(color),
                ),
              ),
              Text('${(value * 100).toInt()}', style: TextStyle(fontSize: 20, fontWeight: FontWeight.w700, color: color)),
            ],
          ),
          const SizedBox(height: 8),
          Text(text, style: TextStyle(fontWeight: FontWeight.w600, color: color, fontSize: 13)),
        ],
      ),
    );
  }

  Widget _buildAnalysisCard(
    BuildContext context, {
    required String symbol,
    required String trend,
    required double confidence,
    required String analysis,
    required Map<String, String> indicators,
    required String action,
  }) {
    final isBullish = trend == 'Bullish';
    final trendColor = isBullish ? AppTheme.primaryGreen : AppTheme.primaryRed;

    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 20, vertical: 6),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.bgCard,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: const Color(0xFF1E293B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(symbol, style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 16, color: AppTheme.textPrimary)),
              const SizedBox(width: 8),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                decoration: BoxDecoration(color: trendColor.withValues(alpha: 0.15), borderRadius: BorderRadius.circular(4)),
                child: Text(trend, style: TextStyle(fontSize: 11, fontWeight: FontWeight.w700, color: trendColor)),
              ),
              const Spacer(),
              Text('${(confidence * 100).toStringAsFixed(0)}%', style: TextStyle(fontWeight: FontWeight.w700, fontSize: 20, color: trendColor)),
            ],
          ),
          const SizedBox(height: 12),
          Text(analysis, style: const TextStyle(color: AppTheme.textSecondary, fontSize: 13, height: 1.5)),
          const SizedBox(height: 12),
          // Indicators row
          Wrap(
            spacing: 8,
            runSpacing: 6,
            children: indicators.entries.map((e) {
              return Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: AppTheme.bgCardLight,
                  borderRadius: BorderRadius.circular(6),
                ),
                child: Text('${e.key}: ${e.value}', style: const TextStyle(fontSize: 11, color: AppTheme.textSecondary)),
              );
            }).toList(),
          ),
        ],
      ),
    );
  }

  Widget _usageMetric(BuildContext context, String label, String value) {
    return Column(
      children: [
        Text(value, style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 18, color: AppTheme.textPrimary)),
        const SizedBox(height: 4),
        Text(label, style: Theme.of(context).textTheme.bodySmall),
      ],
    );
  }
}
