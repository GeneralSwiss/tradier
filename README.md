# Tradier: Rust Library for Tradier Broker API

[![Dual License](https://img.shields.io/badge/license-MIT%20and%20Apache%202.0-blue)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/tradier.svg)](https://crates.io/crates/tradier)
[![Downloads](https://img.shields.io/crates/d/tradier.svg)](https://crates.io/crates/tradier)
[![Stars](https://img.shields.io/github/stars/joaquinbejar/tradier.svg)](https://github.com/joaquinbejar/tradier/stargazers)

[![Build Status](https://img.shields.io/github/workflow/status/joaquinbejar/tradier/CI)](https://github.com/joaquinbejar/tradier/actions)
[![Coverage](https://img.shields.io/codecov/c/github/joaquinbejar/tradier)](https://codecov.io/gh/joaquinbejar/tradier)
[![Dependencies](https://img.shields.io/librariesio/github/joaquinbejar/tradier)](https://libraries.io/github/joaquinbejar/tradier)

## Table of Contents
1. [Introduction](#introduction)
2. [Features](#features)
3. [Project Structure](#project-structure)
4. [Setup Instructions](#setup-instructions)
5. [Library Usage](#library-usage)
6. [Usage Examples](#usage-examples)
7. [Development](#development)
8. [Contribution and Contact](#contribution-and-contact)

## Introduction

tradier is a comprehensive Rust library for managing trades and market data using the Tradier broker API. This powerful toolkit enables developers, traders, and financial applications to:

- Execute trades efficiently
- Retrieve real-time quotes
- Manage portfolios
- Access historical market data

The library leverages Rust's performance and concurrency advantages, making it suitable for high-frequency trading applications and data-intensive financial processing.

## Features

1. **Trade Execution**: Implement order placement, modification, and cancellation.
2. **Real-time Market Data**: Access live quotes, order book data, and trade information.
3. **Portfolio Management**: Retrieve account information, positions, and performance metrics.
4. **Historical Data**: Fetch and analyze historical price and volume data.
5. **Streaming Data**: Utilize WebSocket connections for real-time data feeds.
6. **Authentication**: Securely manage API keys and authentication tokens.
7. **Error Handling**: Robust error handling and logging for reliability.
8. **Rate Limiting**: Implement rate limiting to comply with API usage restrictions.
9. **Concurrent Processing**: Leverage Rust's async capabilities for efficient data handling.
10. **Data Serialization**: Use Serde for efficient JSON parsing and serialization.

## Project Structure

The project is structured as follows:

1. **Root Directory**:
    - `Cargo.lock` and `Cargo.toml`: Rust package manager files for dependency management.
    - `Docker`: Directory containing Docker-related files for containerization.
    - `LICENSE`: The license file for the project.
    - `Makefile`: Contains commands for common development tasks.
    - `README.md`: The main documentation file you're currently reading.
    - `coverage`: Directory for test coverage reports.
    - `rust-toolchain.toml`: Specifies the Rust toolchain version for the project.
    - `tarpaulin-report.html`: HTML report of test coverage generated by cargo-tarpaulin.

2. **Documentation** (`doc/`):
    - `images/`: Directory for storing images used in documentation.

3. **Examples** (`examples/`):
    - `auth_example.rs`: Example of how to use authentication in the library.
    - `auth_websocket_example.rs`: Example of using authenticated WebSocket connections.

4. **Source Code** (`src/`):
    - **Configuration** (`config/`):
        - `base.rs`: Defines the base configuration structure for the library.
        - `mod.rs`: Module declaration file for the config module.
    - `constants.rs`: Defines constant values used throughout the library.
    - `lib.rs`: The main library file that exposes the public API.
    - **Utilities** (`utils/`):
        - `error.rs`: Defines custom error types for the library.
        - `logger.rs`: Sets up logging functionality.
        - `mod.rs`: Module declaration file for the utils module.
        - `tests.rs`: Contains tests for utility functions.
    - **WebSocket Session** (`wssession/`):
        - `account.rs`: Handles account-related WebSocket sessions.
        - `market.rs`: Manages market data WebSocket sessions.
        - `mod.rs`: Module declaration file for the wssession module.
        - `session.rs`: Defines the base WebSocket session structure and functionality.

5. **Tests** (`tests/`):
    - **Unit Tests** (`unit/`):
        - `mod.rs`: Module file for unit tests.

This structure organizes the project into logical components:
- The root directory contains project-wide configuration and documentation files.
- The `examples` directory provides sample code for users to understand how to use the library.
- The `src` directory contains the main library code, divided into modules for configuration, utilities, and WebSocket session management.
- The `tests` directory is set up for containing unit tests, ensuring the reliability of the library's components.

Each module and file has a specific purpose, contributing to a well-organized and maintainable codebase for the TradierRust library.


## Setup Instructions

1. Add tradier to your `Cargo.toml`:
   ```toml
   [dependencies]
   tradier = "0.1.0"
   ```

2. Set up your Tradier API credentials:
   - Create a `.env` file in your project root:
     ```
     TRADIER_ACCESS_TOKEN=your_api_key_here
     TRADIER_CLIENT_ID=your_account_id_here
     ```

3. Build your project:
   ```shell
   cargo build
   ```

## Library Usage

To use the library in your project:

```rust
use tradier::config::base::Config;
use tradier::utils::logger::setup_logger;
use tradier::wssession::market::MarketSession;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    let config = Config::new();
    let market_session = MarketSession::new(&config).await?;
    // Use the market_session to interact with the Tradier API
    Ok(())
}
```

## Usage Examples

Here's an example of how to use the library for streaming market data:

```rust
use std::error::Error;
use tracing::{error, info};
use tradier::config::base::Config;
use tradier::utils::logger::setup_logger;
use tradier::wssession::account::AccountSession;
use tradier::wssession::market::{MarketSession, MarketSessionFilter, MarketSessionPayload};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_logger();
    let config = Config::new();

    loop {
        match MarketSession::new(&config).await {
            Ok(market_session) => {
                info!(
                    "Market streaming session created with id: {}",
                    market_session.get_session_id()
                );
                let payload = MarketSessionPayload {
                    symbols: vec!["AAPL".to_string(), "MSFT".to_string()],
                    filter: Some(vec![MarketSessionFilter::QUOTE, MarketSessionFilter::TRADE]),
                    session_id: market_session.get_session_id().to_string(),
                    linebreak: Some(true),
                    valid_only: Some(true),
                    advanced_details: None,
                };
                if let Err(e) = market_session.ws_stream(payload).await {
                    error!("Streaming error: {}. Reconnecting...", e);
                }
            }
            Err(e) => {
                error!(
                    "Failed to create market streaming session: {}. Retrying...",
                    e
                );
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}
```

## Development

This project includes a Makefile for common development tasks:

- `make build`: Build the project
- `make test`: Run tests
- `make fmt`: Format the code
- `make lint`: Run Clippy for linting
- `make clean`: Clean the project
- `make check`: Run pre-push checks (test, format check, lint)
- `make coverage`: Generate test coverage report
- `make doc`: Generate and open documentation

To run a specific task, use `make <task_name>`. For example:

```shell
make test
```

## Contribution and Contact

We welcome contributions to this project! If you would like to contribute, please follow these steps:

1. Fork the repository.
2. Create a new branch for your feature or bug fix.
3. Make your changes and ensure that the project still builds and all tests pass.
4. Commit your changes and push your branch to your forked repository.
5. Submit a pull request to the main repository.

If you have any questions, issues, or would like to provide feedback, please feel free to contact the project maintainer:

**Joaquín Béjar García**
- Email: jb@taunais.com
- GitHub: [joaquinbejar](https://github.com/joaquinbejar)

We appreciate your interest and look forward to your contributions!
