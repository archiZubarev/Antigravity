# Antigravity Unlocker

Точный приборный инструмент для разблокировки и настройки **Google Antigravity** в России без VPN, смены региона аккаунта и сторонних каналов.

Интерфейс приложения выполнен в строгой приборной тёмной эстетике [Goniometer](https://github.com/archiZubarev/goniometer) на базе **Tauri v2** (React 19 + TypeScript + Rust).

---

## 📥 Где скачать

Последние готовые сборки всегда доступны в разделе **[Releases](https://github.com/archiZubarev/Antigravity/releases/latest)**:

| Платформа | Формат | Ссылка |
| :--- | :--- | :--- |
| **macOS (Apple Silicon)** | `.dmg` | [Скачать для Apple Silicon](https://github.com/archiZubarev/Antigravity/releases/latest) |
| **Windows 10 / 11** | `.exe` / setup | [Скачать для Windows](https://github.com/archiZubarev/Antigravity/releases/latest) |
| **Linux (x86_64)** | `.AppImage` / `.deb` | [Скачать для Linux](https://github.com/archiZubarev/Antigravity/releases/latest) |

### Быстрый запуск в терминале без графического интерфейса (TUI)

**Linux (одной командой без root):**
```bash
curl -fsSL https://raw.githubusercontent.com/archiZubarev/Antigravity/main/tui.sh | sh
```

**Windows PowerShell:**
```powershell
irm https://raw.githubusercontent.com/archiZubarev/Antigravity/main/tui.ps1 | iex
```

---

## ⚡️ Принцип работы

1. **Бинарный патч клиента** — снимает региональные флаги с бинарного файла `language_server` / `agy` и направляет служебный трафик на изолированную локальную точку входа `AG_LS_PROXY`.
2. **Фоновый сторож (Watchdog)**:
   - macOS: системная служба `LaunchAgent` (`~/Library/LaunchAgents/`).
   - Linux: служба `systemd --user`.
   - Windows: фоновая служба планировщика.
   Автоматически удерживает и восстанавливает патч при обновлениях самого Antigravity.
3. **DNS / DoH обход** — шифрованное разрешение служебных доменов Google через доверенные DNS-over-HTTPS резолверы (Google, Cloudflare, Quad9).
4. **Локальный прокси** — внутренний сервис на `127.0.0.1:53129`, маршрутизирующий только вызовы расширения без изменения глобальных системных настроек сети.
5. **Тонкая настройка** — поддержка собственного SOCKS5/HTTP прокси, ротация DNS-резолверов и проверка сертификатов TLS.

---

## 🛠 Сборка из исходников

### Требования
- **Node.js** 20+ и **npm**
- **Rust** (stable toolchain)

### Сборка приложения
```bash
# Установка зависимостей интерфейса
npm install

# Сборка desktop-приложения
npm run tauri build
```

Собранные бинарники и пакеты будут находиться в папке `target/release/bundle/`.

---

## 📄 Лицензия
Non-Commercial & Restricted Use License.
