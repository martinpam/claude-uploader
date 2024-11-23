# Claude.ai File Uploader

The Claude.ai File Uploader is a desktop application that makes it easy to upload files to your Claude.ai projects. Since the Claude.ai web interface doesn't allow uploading entire folders, this tool solves that problem by letting you select a folder and uploading all the supported files in it.

## Key Features:
- Uploads files to your Claude.ai projects efficiently
- Automatically skips files listed in your .gitignore
- Provides detailed upload status and error reporting
- Handles large file uploads without issues
- Clean and intuitive user interface

## Installation
To use the Claude.ai File Uploader, you'll need to have Rust installed on your system. If you don't have Rust installed, you can download it from the official Rust website: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)

Once you have Rust installed, you can clone the repository and build the application:
```bash
git clone https://github.com/martinpam/claude-uploader.git
cd claude-uploader
cargo build --release
```

This will create an executable file in the `target/release` directory. You can then run the application using the following command:

```bash
./target/release/claude_uploader
```

Alternatively, you can run the application directly with Cargo:

```bash
git clone https://github.com/martinpam/claude-uploader.git
cd claude-uploader
cargo run
```
The `cargo run` command will build and run the application without creating a separate executable file, which may result in a smaller file size.


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
This application is provided as-is, I am not responsible for any issues or problems that may arise from its use. Please review the source code and ensure that you understand what the application is doing before using it.

## License
This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for more information.
