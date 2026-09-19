import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/theme.dart';

class PositionsScreen extends ConsumerWidget {
  const PositionsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return SafeArea(
      child: DefaultTabController(
        length: 2,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text('Positions', style: Theme.of(context).textTheme.displayMedium),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                    decoration: BoxDecoration(
                      color: AppTheme.primaryGreen.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Text(
                      'P&L: +\$125.43',
                      style: TextStyle(color: AppTheme.primaryGreen, fontWeight: FontWeight.w600, fontSize: 13),
                    ),
                  ),
                ],
              ),
            ),

            // Tab bar
            const Padding(
              padding: EdgeInsets.fromLTRB(20, 16, 20, 0),
              child: TabBar(
                indicatorColor: AppTheme.accentBlue,
                labelColor: AppTheme.textPrimary,
                unselectedLabelColor: AppTheme.textMuted,
                tabs: [
                  Tab(text: 'Open (3)'),
                  Tab(text: 'History'),
                ],
              ),
            ),

            Expanded(
              child: TabBarView(
                children: [
                  // Open positions
                  ListView(
                    padding: const EdgeInsets.all(20),
                    children: [
                      _buildDetailedPosition(
                        context, 'BTCUSDT', 'MEXC', 'LONG',
                        entry: 64250.00, current: 64820.50,
                        quantity: 0.015, sl: 63800.00, tp: 65500.00,
                        pnl: 57.05, pnlPct: 0.89, time: '2h 15m',
                      ),
                      _buildDetailedPosition(
                        context, 'ETHUSDT', 'MEXC', 'LONG',
                        entry: 3420.00, current: 3455.80,
                        quantity: 0.5, sl: 3380.00, tp: 3520.00,
                        pnl: 35.80, pnlPct: 1.05, time: '45m',
                      ),
                      _buildDetailedPosition(
                        context, 'SOLUSDT', 'MEXC', 'SHORT',
                        entry: 142.50, current: 140.20,
                        quantity: 10.0, sl: 145.00, tp: 136.00,
                        pnl: 23.00, pnlPct: 1.61, time: '1h 30m',
                      ),
                    ],
                  ),
                  // Trade history
                  ListView(
                    padding: const EdgeInsets.all(20),
                    children: [
                      _buildHistoryItem(context, 'BTCUSDT', 'LONG', 63500.00, 64100.00, 42.00, 'Take Profit', '3h ago'),
                      _buildHistoryItem(context, 'AVAXUSDT', 'SHORT', 28.50, 29.10, -12.00, 'Stop Loss', '5h ago'),
                      _buildHistoryItem(context, 'ETHUSDT', 'LONG', 3350.00, 3410.00, 30.00, 'Take Profit', '8h ago'),
                      _buildHistoryItem(context, 'SOLUSDT', 'LONG', 138.00, 141.50, 35.00, 'Manual Close', '1d ago'),
                    ],
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildDetailedPosition(
    BuildContext context, String symbol, String exchange, String side, {
    required double entry, required double current, required double quantity,
    required double sl, required double tp,
    required double pnl, required double pnlPct, required String time,
  }) {
    final isLong = side == 'LONG';
    final isProfit = pnl > 0;

    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.bgCard,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: const Color(0xFF1E293B)),
      ),
      child: Column(
        children: [
          // Header row
          Row(
            children: [
              Text(symbol, style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 16, color: AppTheme.textPrimary)),
              const SizedBox(width: 8),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                decoration: BoxDecoration(
                  color: (isLong ? AppTheme.primaryGreen : AppTheme.primaryRed).withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(side, style: TextStyle(fontSize: 11, fontWeight: FontWeight.w700, color: isLong ? AppTheme.primaryGreen : AppTheme.primaryRed)),
              ),
              const Spacer(),
              Text(
                '${isProfit ? '+' : ''}\$${pnl.toStringAsFixed(2)} (${pnlPct.toStringAsFixed(2)}%)',
                style: TextStyle(fontWeight: FontWeight.w700, fontSize: 16, color: isProfit ? AppTheme.primaryGreen : AppTheme.primaryRed),
              ),
            ],
          ),
          const SizedBox(height: 12),
          const Divider(color: Color(0xFF1E293B), height: 1),
          const SizedBox(height: 12),
          // Details grid
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              _infoCol('Entry', '\$${entry.toStringAsFixed(2)}', context),
              _infoCol('Current', '\$${current.toStringAsFixed(2)}', context),
              _infoCol('Qty', quantity.toStringAsFixed(4), context),
              _infoCol('Duration', time, context),
            ],
          ),
          const SizedBox(height: 8),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              _infoCol('Stop Loss', '\$${sl.toStringAsFixed(2)}', context, color: AppTheme.primaryRed),
              _infoCol('Take Profit', '\$${tp.toStringAsFixed(2)}', context, color: AppTheme.primaryGreen),
              // Close button
              ElevatedButton.icon(
                onPressed: () {},
                icon: const Icon(Icons.close, size: 16),
                label: const Text('Close', style: TextStyle(fontSize: 12)),
                style: ElevatedButton.styleFrom(
                  backgroundColor: AppTheme.primaryRed.withValues(alpha: 0.2),
                  foregroundColor: AppTheme.primaryRed,
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  minimumSize: Size.zero,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _infoCol(String label, String value, BuildContext context, {Color? color}) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: Theme.of(context).textTheme.bodySmall),
        const SizedBox(height: 2),
        Text(value, style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13, color: color ?? AppTheme.textPrimary)),
      ],
    );
  }

  Widget _buildHistoryItem(BuildContext context, String symbol, String side, double entry, double exit, double pnl, String reason, String time) {
    final isProfit = pnl > 0;
    final isLong = side == 'LONG';

    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppTheme.bgCard,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF1E293B)),
      ),
      child: Row(
        children: [
          Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: (isProfit ? AppTheme.primaryGreen : AppTheme.primaryRed).withValues(alpha: 0.15),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Icon(
              isProfit ? Icons.check_circle_rounded : Icons.cancel_rounded,
              color: isProfit ? AppTheme.primaryGreen : AppTheme.primaryRed,
              size: 20,
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(symbol, style: const TextStyle(fontWeight: FontWeight.w600, color: AppTheme.textPrimary)),
                    const SizedBox(width: 6),
                    Text(side, style: TextStyle(fontSize: 11, color: isLong ? AppTheme.primaryGreen : AppTheme.primaryRed)),
                  ],
                ),
                Text('$reason · $time', style: Theme.of(context).textTheme.bodySmall),
              ],
            ),
          ),
          Text(
            '${isProfit ? '+' : ''}\$${pnl.toStringAsFixed(2)}',
            style: TextStyle(fontWeight: FontWeight.w700, color: isProfit ? AppTheme.primaryGreen : AppTheme.primaryRed),
          ),
        ],
      ),
    );
  }
}
