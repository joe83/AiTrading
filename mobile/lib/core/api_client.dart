import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// API client for communicating with the Rust trading server.
class ApiClient {
  final Dio _dio;

  ApiClient({String baseUrl = 'http://localhost:8080'})
      : _dio = Dio(BaseOptions(
          baseUrl: baseUrl,
          connectTimeout: const Duration(seconds: 10),
          receiveTimeout: const Duration(seconds: 30),
          headers: {
            'Content-Type': 'application/json',
          },
        ));

  // =========================================================================
  // Dashboard
  // =========================================================================

  Future<Map<String, dynamic>> getDashboard() async {
    final response = await _dio.get('/api/dashboard');
    return response.data;
  }

  // =========================================================================
  // Positions
  // =========================================================================

  Future<Map<String, dynamic>> getPositions() async {
    final response = await _dio.get('/api/positions');
    return response.data;
  }

  // =========================================================================
  // Signals
  // =========================================================================

  Future<Map<String, dynamic>> getSignals() async {
    final response = await _dio.get('/api/signals');
    return response.data;
  }

  Future<Map<String, dynamic>> getPendingSignals() async {
    final response = await _dio.get('/api/signals/pending');
    return response.data;
  }

  Future<void> approveSignal(String signalId) async {
    await _dio.post('/api/signals/$signalId/approve');
  }

  Future<void> rejectSignal(String signalId) async {
    await _dio.post('/api/signals/$signalId/reject');
  }

  // =========================================================================
  // Analysis
  // =========================================================================

  Future<Map<String, dynamic>> getAnalysis(String symbol) async {
    final response = await _dio.get('/api/analysis/$symbol');
    return response.data;
  }

  // =========================================================================
  // Performance
  // =========================================================================

  Future<Map<String, dynamic>> getPerformance() async {
    final response = await _dio.get('/api/performance');
    return response.data;
  }

  Future<Map<String, dynamic>> getTradeHistory() async {
    final response = await _dio.get('/api/trades');
    return response.data;
  }

  // =========================================================================
  // Settings & Control
  // =========================================================================

  Future<void> setTradingMode(String mode) async {
    await _dio.post('/api/settings/mode', data: {'mode': mode});
  }

  Future<Map<String, dynamic>> getTradingMode() async {
    final response = await _dio.get('/api/settings/mode');
    return response.data;
  }

  Future<void> pauseTrading() async {
    await _dio.post('/api/control/pause');
  }

  Future<void> resumeTrading() async {
    await _dio.post('/api/control/resume');
  }

  // =========================================================================
  // System
  // =========================================================================

  Future<Map<String, dynamic>> getHealth() async {
    final response = await _dio.get('/api/health');
    return response.data;
  }

  Future<Map<String, dynamic>> getStatus() async {
    final response = await _dio.get('/api/status');
    return response.data;
  }
}

/// Provider for the API client.
final apiClientProvider = Provider<ApiClient>((ref) {
  return ApiClient();
});
