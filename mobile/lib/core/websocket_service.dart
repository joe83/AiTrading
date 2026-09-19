import 'dart:async';
import 'dart:convert';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

/// WebSocket service singleton for real-time data from the trading server.
class WebSocketService {
  WebSocketChannel? _channel;
  final StreamController<Map<String, dynamic>> _controller =
      StreamController<Map<String, dynamic>>.broadcast();
  bool _isConnected = false;
  int _reconnectAttempts = 0;
  Timer? _reconnectTimer;
  Timer? _heartbeatTimer;

  String _serverUrl;

  WebSocketService({String serverUrl = 'ws://localhost:8081/ws'})
      : _serverUrl = serverUrl;

  /// Stream of parsed JSON messages from the server.
  Stream<Map<String, dynamic>> get messages => _controller.stream;

  /// Whether the WebSocket is currently connected.
  bool get isConnected => _isConnected;

  /// Connect to the trading server WebSocket.
  void connect() {
    try {
      _channel = WebSocketChannel.connect(Uri.parse(_serverUrl));

      _channel!.stream.listen(
        (data) {
          _isConnected = true;
          _reconnectAttempts = 0;

          if (data is String) {
            try {
              final json = jsonDecode(data) as Map<String, dynamic>;
              _controller.add(json);
            } catch (e) {
              // Ignore malformed messages
            }
          }
        },
        onError: (error) {
          _isConnected = false;
          _scheduleReconnect();
        },
        onDone: () {
          _isConnected = false;
          _scheduleReconnect();
        },
      );

      // Start heartbeat
      _heartbeatTimer?.cancel();
      _heartbeatTimer = Timer.periodic(const Duration(seconds: 30), (_) {
        send({'action': 'ping'});
      });
    } catch (e) {
      _isConnected = false;
      _scheduleReconnect();
    }
  }

  /// Send a message to the server.
  void send(Map<String, dynamic> message) {
    if (_isConnected && _channel != null) {
      _channel!.sink.add(jsonEncode(message));
    }
  }

  /// Subscribe to specific symbols.
  void subscribeSymbols(List<String> symbols) {
    send({
      'action': 'subscribe',
      'symbols': symbols,
    });
  }

  /// Disconnect and clean up.
  void disconnect() {
    _heartbeatTimer?.cancel();
    _reconnectTimer?.cancel();
    _channel?.sink.close();
    _isConnected = false;
  }

  /// Schedule reconnection with exponential backoff.
  void _scheduleReconnect() {
    _reconnectTimer?.cancel();
    final delay = Duration(
      seconds: (1 << _reconnectAttempts.clamp(0, 6)), // max 64 seconds
    );
    _reconnectAttempts++;

    _reconnectTimer = Timer(delay, () {
      connect();
    });
  }

  /// Update the server URL (e.g., from settings).
  void updateServerUrl(String url) {
    _serverUrl = url;
    disconnect();
    connect();
  }
}

/// Provider for the WebSocket service.
final wsServiceProvider = Provider<WebSocketService>((ref) {
  final service = WebSocketService();
  service.connect();
  ref.onDispose(() => service.disconnect());
  return service;
});

/// Stream provider for real-time WebSocket messages.
final wsMessagesProvider = StreamProvider<Map<String, dynamic>>((ref) {
  final service = ref.watch(wsServiceProvider);
  return service.messages;
});
