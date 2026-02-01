# <img src="public/logo.png" width="48" align="center" /> PG-Migrate

**PG-Migrate** is a high-performance, cross-platform PostgreSQL table migration tool built with **Rust**, **Tauri**, and **React**. It is designed to be the fastest way to clone tables, schemas, and data between PostgreSQL databases with zero dependencies required on your system.

## 🚀 Key Features

-   **Turbo Batching**: Optimized multi-row `INSERT` logic that is up to 50x faster than standard migration tools.
-   **Keyset Pagination**: Uses Seek-based pagination (`WHERE id > last_val`) instead of slow `OFFSET`, ensuring constant speed even for tables with millions of rows.
-   **JSON/JSONB Support**: High-fidelity transfer of complex metadata and document data.
-   **Schema Remapping**: Easily move tables between different schemas (e.g., from `public` to `archive`) during the migration process.
-   **Sequence Synchronization**: Automatically resets `SERIAL` and `BIGSERIAL` sequences on the target database to prevent unique constraint violations.
-   **Safety First**: Non-destructive, high-fidelity cloning. Supports `ON CONFLICT DO NOTHING` and optional table truncation.
-   **Bidirectional direction**: Swap source and target with a single click.
-   **Cross-Platform**: Native executables for **Windows**, **Linux**, and **macOS**.

## 🛠️ Tech Stack

-   **Backend**: Rust with `sqlx` (Asynchronous PostgreSQL driver).
-   **Frontend**: React + TypeScript + Vite.
-   **Bridge**: Tauri (Smallest, fastest, and most secure desktop app framework).

## 📦 Installation

### Download Executables
You can download the pre-compiled executables for your operating system from the [Releases](https://github.com/YOUR_USERNAME/pg-migrate/releases) page.

-   **Windows**: `.exe` installer or portable binary.
-   **Linux**: `.AppImage` or `.deb` package.
-   **macOS**: `.dmg` (Universal, Intel/Apple Silicon).

### Build from Source
If you prefer to build it yourself, ensure you have **Node.js** and **Rust** installed on your system.

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/pg-migrate.git
cd pg-migrate

# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## 🐧 Linux Support
On Linux, you may need to install the following dependencies for the webview to work:
```bash
sudo apt-get update
sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.0-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
```

## 📄 License
MIT License - feel free to use and contribute!
