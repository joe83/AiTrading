import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/theme.dart';

class SettingsScreen extends ConsumerStatefulWidget {
  const SettingsScreen({super.key});

  @override
  ConsumerState<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends ConsumerState<SettingsScreen> {
  bool isAutoMode = false;
  bool isPaused = false;
  double maxPositionSize = 5.0;
  double maxDailyLoss = 3.0;
  double defaultStopLoss = 2.0;
  double defaultTakeProfit = 4.0;
  int maxConcurrentPositions = 5;
  bool notificationsEnabled = true;
  bool biometricAuth = false;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ListView(
        physics: const BouncingScrollPhysics(),
        padding: const EdgeInsets.all(20),
        children: [
          Text('Settings', style: Theme.of(context).textTheme.displayMedium),
          const SizedBox(height: 24),

          // Trading Mode
          _sectionTitle('Trading Mode'),
          _buildCard([
            _switchTile(
              'Auto Trading',
              'AI executes trades automatically',
              isAutoMode,
              (v) => setState(() => isAutoMode = v),
              icon: Icons.smart_toy_rounded,
            ),
            const Divider(color: Color(0xFF1E293B), height: 1),
            _switchTile(
              'Pause Trading',
              'Stop all new trades temporarily',
              isPaused,
              (v) => setState(() => isPaused = v),
              icon: Icons.pause_circle_rounded,
              activeColor: AppTheme.primaryRed,
            ),
          ]),
          const SizedBox(height: 24),

          // Risk Management
          _sectionTitle('Risk Management'),
          _buildCard([
            _sliderTile(
              'Max Position Size',
              '${maxPositionSize.toStringAsFixed(1)}% of portfolio',
              maxPositionSize, 1.0, 20.0,
              (v) => setState(() => maxPositionSize = v),
            ),
            const Divider(color: Color(0xFF1E293B), height: 1),
            _sliderTile(
              'Max Daily Loss',
              '${maxDailyLoss.toStringAsFixed(1)}% of portfolio',
              maxDailyLoss, 1.0, 10.0,
              (v) => setState(() => maxDailyLoss = v),
            ),
            const Divider(color: Color(0xFF1E293B), height: 1),
            _sliderTile(
              'Default Stop Loss',
              '${defaultStopLoss.toStringAsFixed(1)}%',
              defaultStopLoss, 0.5, 10.0,
              (v) => setState(() => defaultStopLoss = v),
            ),
            const Divider(color: Color(0xFF1E293B), height: 1),
            _sliderTile(
              'Default Take Profit',
              '${defaultTakeProfit.toStringAsFixed(1)}%',
              defaultTakeProfit, 1.0, 20.0,
              (v) => setState(() => defaultTakeProfit = v),
            ),
            const Divider(color: Color(0xFF1E293B), height: 1),
            ListTile(
              title: const Text('Max Concurrent Positions', style: TextStyle(color: AppTheme.textPrimary, fontSize: 14)),
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  IconButton(
                    icon: const Icon(Icons.remove_circle_outline, color: AppTheme.textSecondary),
                    onPressed: maxConcurrentPositions > 1
                        ? () => setState(() => maxConcurrentPositions--)
                        : null,
                  ),
                  Text('$maxConcurrentPositions', style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 18, color: AppTheme.textPrimary)),
                  IconButton(
                    icon: const Icon(Icons.add_circle_outline, color: AppTheme.textSecondary),
                    onPressed: () => setState(() => maxConcurrentPositions++),
                  ),
                ],
              ),
            ),
          ]),
          const SizedBox(height: 24),

          // Notifications
          _sectionTitle('Notifications'),
          _buildCard([
            _switchTile(
              'Push Notifications',
              'Trade alerts and signal notifications',
              notificationsEnabled,
              (v) => setState(() => notificationsEnabled = v),
              icon: Icons.notifications_rounded,
            ),
          ]),
          const SizedBox(height: 24),

          // Security
          _sectionTitle('Security'),
          _buildCard([
            _switchTile(
              'Biometric Authentication',
              'Require fingerprint or face ID',
              biometricAuth,
              (v) => setState(() => biometricAuth = v),
              icon: Icons.fingerprint_rounded,
            ),
          ]),
          const SizedBox(height: 24),

          // Server Connection
          _sectionTitle('Server Connection'),
          _buildCard([
            ListTile(
              leading: const Icon(Icons.cloud_done_rounded, color: AppTheme.primaryGreen),
              title: const Text('Server Status', style: TextStyle(color: AppTheme.textPrimary, fontSize: 14)),
              subtitle: const Text('Connected · localhost:8080', style: TextStyle(color: AppTheme.textSecondary, fontSize: 12)),
              trailing: const Icon(Icons.chevron_right_rounded, color: AppTheme.textMuted),
            ),
            const Divider(color: Color(0xFF1E293B), height: 1),
            ListTile(
              leading: const Icon(Icons.key_rounded, color: AppTheme.accentBlue),
              title: const Text('API Keys', style: TextStyle(color: AppTheme.textPrimary, fontSize: 14)),
              subtitle: const Text('Manage exchange API keys', style: TextStyle(color: AppTheme.textSecondary, fontSize: 12)),
              trailing: const Icon(Icons.chevron_right_rounded, color: AppTheme.textMuted),
              onTap: () {},
            ),
          ]),
          const SizedBox(height: 24),

          // About
          _sectionTitle('About'),
          _buildCard([
            const ListTile(
              leading: Icon(Icons.info_outline_rounded, color: AppTheme.textSecondary),
              title: Text('Version', style: TextStyle(color: AppTheme.textPrimary, fontSize: 14)),
              trailing: Text('1.0.0', style: TextStyle(color: AppTheme.textSecondary)),
            ),
            const Divider(color: Color(0xFF1E293B), height: 1),
            const ListTile(
              leading: Icon(Icons.psychology_rounded, color: AppTheme.accentPurple),
              title: Text('AI Model', style: TextStyle(color: AppTheme.textPrimary, fontSize: 14)),
              trailing: Text('Grok 4.6', style: TextStyle(color: AppTheme.textSecondary)),
            ),
          ]),

          const SizedBox(height: 100),
        ],
      ),
    );
  }

  Widget _sectionTitle(String title) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Text(title, style: const TextStyle(color: AppTheme.textSecondary, fontWeight: FontWeight.w600, fontSize: 13, letterSpacing: 0.5)),
    );
  }

  Widget _buildCard(List<Widget> children) {
    return Container(
      decoration: BoxDecoration(
        color: AppTheme.bgCard,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: const Color(0xFF1E293B)),
      ),
      child: Column(children: children),
    );
  }

  Widget _switchTile(String title, String subtitle, bool value, ValueChanged<bool> onChanged, {IconData? icon, Color? activeColor}) {
    return ListTile(
      leading: icon != null ? Icon(icon, color: value ? (activeColor ?? AppTheme.accentBlue) : AppTheme.textMuted) : null,
      title: Text(title, style: const TextStyle(color: AppTheme.textPrimary, fontSize: 14)),
      subtitle: Text(subtitle, style: const TextStyle(color: AppTheme.textSecondary, fontSize: 12)),
      trailing: Switch.adaptive(
        value: value,
        onChanged: onChanged,
        activeTrackColor: activeColor ?? AppTheme.accentBlue,
      ),
    );
  }

  Widget _sliderTile(String title, String valueText, double value, double min, double max, ValueChanged<double> onChanged) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(title, style: const TextStyle(color: AppTheme.textPrimary, fontSize: 14)),
              Text(valueText, style: const TextStyle(color: AppTheme.accentBlue, fontWeight: FontWeight.w600, fontSize: 13)),
            ],
          ),
          Slider(
            value: value,
            min: min,
            max: max,
            divisions: ((max - min) * 2).round(),
            activeColor: AppTheme.accentBlue,
            inactiveColor: const Color(0xFF1E293B),
            onChanged: onChanged,
          ),
        ],
      ),
    );
  }
}
