der # Claude.ai File Uploader

## Features
- Supports a wide range of file types, including code files, documentation, and media files
- Automatically skips files listed in .gitignore
- Provides detailed upload status and error reporting
- Handles large file uploads without issues
- Clean and intuitive user interface

## Installation
To use the Claude.ai File Uploader, you'll need to have Rust installed on your system. If you don't have Rust installed, you can download it from the official Rust website: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)

Once you have Rust installed, you can clone the repository and build the application:

```bash
git clone https://github.com/OnePromptMagic/claude-uploader.git
cd claude-uploader
cargo build --release
```

This will create an executable file in the `target/release` directory. You can then run the application using the following command:

```bash
./target/release/claude_uploader
```

## Usage
1. Copy the cURL request from the Claude.ai website:
   - Open the browser's developer tools (F12)
   - Go to the Network tab
   - Upload a file manually on Claude.ai
   - Find the upload request (usually the first 'docs' request)
   - Right-click and select "Copy as cURL"

2. Paste the cURL request into the input field in the application.
3. Select the folder containing the files you want to upload.
4. Click the "Upload Files" button to begin the upload process.

The application will display the upload progress, as well as any errors or skipped files. You can view the detailed file status by clicking the "Show Details" button.

## Disclaimer
This application is provided as-is, and the developers are not responsible for any issues or problems that may arise from its use. Please review the source code and ensure that you understand what the application is doing before using it.

## License
This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for more information.
