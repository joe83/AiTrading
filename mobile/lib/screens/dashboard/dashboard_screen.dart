import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/theme.dart';

class DashboardScreen extends ConsumerStatefulWidget {
  const DashboardScreen({super.key});

  @override
  ConsumerState<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends ConsumerState<DashboardScreen> {
  // Demo data — will be replaced with real data from API
  final double totalBalance = 10425.67;
  final double dailyPnl = 125.43;
  final double dailyPnlPct = 1.22;
  final double weeklyPnl = 432.80;
  final int openPositions = 3;
  final int pendingSignals = 2;
  final double winRate = 68.5;
  final bool isAutoMode = false;
  final bool isPaused = false;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: CustomScrollView(
        physics: const BouncingScrollPhysics(),
        slivers: [
          // Header
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'AI Trading',
                        style: Theme.of(context).textTheme.displayMedium,
                      ),
                      const SizedBox(height: 4),
                      Row(
                        children: [
                          Container(
                            width: 8,
                            height: 8,
                            decoration: BoxDecoration(
                              color: isPaused ? AppTheme.primaryRed : AppTheme.primaryGreen,
                              shape: BoxShape.circle,
                            ),
                          ),
                          const SizedBox(width: 6),
                          Text(
                            isPaused ? 'Paused' : (isAutoMode ? 'Auto Trading' : 'Manual Mode'),
                            style: Theme.of(context).textTheme.bodyMedium,
                          ),
                        ],
                      ),
                    ],
                  ),
                  // Emergency close all button
                  Container(
                    decoration: BoxDecoration(
                      border: Border.all(color: AppTheme.primaryRed.withValues(alpha: 0.3)),
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: IconButton(
                      icon: const Icon(Icons.power_settings_new_rounded, color: AppTheme.primaryRed),
                      onPressed: () {
                        _showEmergencyDialog(context);
                      },
                    ),
                  ),
                ],
              ),
            ),
          ),

          // Balance Card
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
              child: _buildBalanceCard(),
            ),
          ),

          // Quick Stats Row
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
              child: Row(
                children: [
                  Expanded(child: _buildStatCard('Open', '$openPositions', Icons.trending_up_rounded, AppTheme.accentBlue)),
                  const SizedBox(width: 12),
                  Expanded(child: _buildStatCard('Signals', '$pendingSignals', Icons.notifications_active_rounded, AppTheme.accentPurple)),
                  const SizedBox(width: 12),
                  Expanded(child: _buildStatCard('Win Rate', '${winRate.toStringAsFixed(1)}%', Icons.emoji_events_rounded, AppTheme.chartYellow)),
                ],
              ),
            ),
          ),

          // Active Positions Section
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 24, 20, 8),
              child: Text(
                'Active Positions',
                style: Theme.of(context).textTheme.titleLarge,
              ),
            ),
          ),

          SliverList(
            delegate: SliverChildListDelegate([
              _buildPositionCard('BTCUSDT', 'MEXC', 'LONG', 64250.00, 64820.50, 0.89, 57.05),
              _buildPositionCard('ETHUSDT', 'MEXC', 'LONG', 3420.00, 3455.80, 1.05, 35.80),
              _buildPositionCard('SOLUSDT', 'MEXC', 'SHORT', 142.50, 140.20, 1.61, 23.00),
            ]),
          ),

          // Recent Signals
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 24, 20, 8),
              child: Text(
                'Recent AI Signals',
                style: Theme.of(context).textTheme.titleLarge,
              ),
            ),
          ),

          SliverList(
            delegate: SliverChildListDelegate([
              _buildSignalCard('BTCUSDT', 'BUY', 0.82, 'Strong bullish momentum with MACD crossover', '2m ago'),
              _buildSignalCard('AVAXUSDT', 'HOLD', 0.54, 'Neutral — waiting for volume confirmation', '15m ago'),
            ]),
          ),

          const SliverToBoxAdapter(child: SizedBox(height: 100)),
        ],
      ),
    );
  }

  Widget _buildBalanceCard() {
    final isPositive = dailyPnl >= 0;

    return Container(
      padding: const EdgeInsets.all(24),
      decoration: BoxDecoration(
        gradient: LinearGradient(
          colors: [
            const Color(0xFF1A2332),
            const Color(0xFF111827),
          ],
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
        ),
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: const Color(0xFF1E293B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Total Balance', style: Theme.of(context).textTheme.bodyMedium),
          const SizedBox(height: 8),
          Text(
            '\$${totalBalance.toStringAsFixed(2)}',
            style: const TextStyle(
              fontSize: 36,
              fontWeight: FontWeight.w700,
              color: AppTheme.textPrimary,
              letterSpacing: -1,
            ),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              // Daily P&L
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                decoration: BoxDecoration(
                  color: (isPositive ? AppTheme.primaryGreen : AppTheme.primaryRed).withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(
                      isPositive ? Icons.trending_up_rounded : Icons.trending_down_rounded,
                      color: isPositive ? AppTheme.primaryGreen : AppTheme.primaryRed,
                      size: 16,
                    ),
                    const SizedBox(width: 4),
                    Text(
                      '${isPositive ? '+' : ''}\$${dailyPnl.toStringAsFixed(2)} (${dailyPnlPct.toStringAsFixed(2)}%)',
                      style: TextStyle(
                        color: isPositive ? AppTheme.primaryGreen : AppTheme.primaryRed,
                        fontWeight: FontWeight.w600,
                        fontSize: 13,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              Text('Today', style: Theme.of(context).textTheme.bodySmall),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildStatCard(String label, String value, IconData icon, Color color) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: const Color(0xFF1E293B)),
      ),
      child: Column(
        children: [
          Icon(icon, color: color, size: 24),
          const SizedBox(height: 8),
          Text(value, style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w700, color: AppTheme.textPrimary)),
          const SizedBox(height: 4),
          Text(label, style: Theme.of(context).textTheme.bodySmall),
        ],
      ),
    );
  }

  Widget _buildPositionCard(String symbol, String exchange, String side, double entry, double current, double pnlPct, double pnlUsd) {
    final isLong = side == 'LONG';
    final isProfit = pnlPct > 0;

    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.bgCard,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: const Color(0xFF1E293B)),
      ),
      child: Row(
        children: [
          // Symbol & side
          Expanded(
            flex: 3,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(symbol, style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 15, color: AppTheme.textPrimary)),
                    const SizedBox(width: 6),
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                      decoration: BoxDecoration(
                        color: (isLong ? AppTheme.primaryGreen : AppTheme.primaryRed).withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        side,
                        style: TextStyle(
                          fontSize: 10,
                          fontWeight: FontWeight.w600,
                          color: isLong ? AppTheme.primaryGreen : AppTheme.primaryRed,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                Text('$exchange · Entry: \$${entry.toStringAsFixed(2)}', style: Theme.of(context).textTheme.bodySmall),
              ],
            ),
          ),
          // Current price & P&L
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text('\$${current.toStringAsFixed(2)}', style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 15, color: AppTheme.textPrimary)),
              const SizedBox(height: 4),
              Text(
                '${isProfit ? '+' : ''}${pnlPct.toStringAsFixed(2)}% (\$${pnlUsd.toStringAsFixed(2)})',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: isProfit ? AppTheme.primaryGreen : AppTheme.primaryRed,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildSignalCard(String symbol, String action, double confidence, String reasoning, String time) {
    Color actionColor;
    IconData actionIcon;
    switch (action) {
      case 'BUY':
      case 'STRONG_BUY':
        actionColor = AppTheme.primaryGreen;
        actionIcon = Icons.arrow_upward_rounded;
        break;
      case 'SELL':
      case 'STRONG_SELL':
        actionColor = AppTheme.primaryRed;
        actionIcon = Icons.arrow_downward_rounded;
        break;
      default:
        actionColor = AppTheme.accentBlue;
        actionIcon = Icons.pause_rounded;
    }

    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
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
              Container(
                padding: const EdgeInsets.all(6),
                decoration: BoxDecoration(
                  color: actionColor.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Icon(actionIcon, color: actionColor, size: 18),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Text(symbol, style: const TextStyle(fontWeight: FontWeight.w600, color: AppTheme.textPrimary)),
                        const SizedBox(width: 6),
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                          decoration: BoxDecoration(
                            color: actionColor.withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: Text(action, style: TextStyle(fontSize: 11, fontWeight: FontWeight.w700, color: actionColor)),
                        ),
                      ],
                    ),
                    const SizedBox(height: 2),
                    Text(time, style: Theme.of(context).textTheme.bodySmall),
                  ],
                ),
              ),
              // Confidence
              Column(
                children: [
                  Text('${(confidence * 100).toStringAsFixed(0)}%', style: TextStyle(fontWeight: FontWeight.w700, color: actionColor, fontSize: 18)),
                  Text('Confidence', style: Theme.of(context).textTheme.bodySmall),
                ],
              ),
            ],
          ),
          const SizedBox(height: 10),
          Text(reasoning, style: Theme.of(context).textTheme.bodyMedium),
        ],
      ),
    );
  }

  void _showEmergencyDialog(BuildContext context) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.bgCard,
        title: const Text('Emergency Close All', style: TextStyle(color: AppTheme.primaryRed)),
        content: const Text('This will immediately close ALL open positions at market price. Are you sure?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Cancel'),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(backgroundColor: AppTheme.primaryRed),
            onPressed: () {
              Navigator.pop(ctx);
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('All positions closed'), backgroundColor: AppTheme.primaryRed),
              );
            },
            child: const Text('CLOSE ALL'),
          ),
        ],
      ),
    );
  }
}
