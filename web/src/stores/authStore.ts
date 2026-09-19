// ===========================================================================
// Auth Store — JWT token management for the web dashboard
// ===========================================================================

import { create } from 'zustand';
import { api } from '../api/client';

interface AuthState {
  token: string | null;
  username: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;

  login: (username: string, password: string) => Promise<boolean>;
  logout: () => void;
  checkAuth: () => Promise<boolean>;
  getToken: () => string | null;
}

const TOKEN_KEY = 'aitrading_token';
const USERNAME_KEY = 'aitrading_user';

export const useAuthStore = create<AuthState>((set, get) => ({
  token: localStorage.getItem(TOKEN_KEY),
  username: localStorage.getItem(USERNAME_KEY),
  isAuthenticated: !!localStorage.getItem(TOKEN_KEY),
  isLoading: false,
  error: null,

  login: async (username: string, password: string) => {
    set({ isLoading: true, error: null });
    try {
      const response = await api.login(username, password);
      const token = response.token;

      localStorage.setItem(TOKEN_KEY, token);
      localStorage.setItem(USERNAME_KEY, username);

      set({
        token,
        username,
        isAuthenticated: true,
        isLoading: false,
        error: null,
      });

      return true;
    } catch (e: any) {
      const message = e?.message || 'Login failed';
      set({
        token: null,
        username: null,
        isAuthenticated: false,
        isLoading: false,
        error: message.includes('401') ? 'Invalid username or password' : message,
      });
      return false;
    }
  },

  logout: () => {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USERNAME_KEY);
    set({
      token: null,
      username: null,
      isAuthenticated: false,
      error: null,
    });
  },

  checkAuth: async () => {
    const token = get().token;
    if (!token) {
      set({ isAuthenticated: false });
      return false;
    }

    try {
      await api.verifyToken();
      return true;
    } catch {
      // Token expired or invalid
      get().logout();
      return false;
    }
  },

  getToken: () => get().token,
}));
