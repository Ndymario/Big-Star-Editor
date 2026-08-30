#pragma once

#include <QObject>
#include <QString>
#include <QWidget>

#include <filesystem>
#include <memory>
#include <thread>

class QComboBox;
class QLabel;
class QProgressBar;
class QPlainTextEdit;
class QPushButton;
class QTreeWidget;
class QTreeWidgetItem;

// Runs one NCPatcher build off the GUI thread.
//
// The backend's build callbacks arrive on the process reader thread. Emitting
// them as signals from here hands them to Qt's queued delivery, so the widgets
// below are only ever touched on the thread that owns them.
class PatcherRunner final : public QObject {
    Q_OBJECT

public:
    explicit PatcherRunner(QObject* parent = nullptr);
    ~PatcherRunner() override;

    [[nodiscard]] bool running() const { return running_; }

    // With an extracted directory the run is a transactional apply: NCPatcher
    // builds to a staging ROM, the result is validated and extracted there, and
    // only then does the live tree get backed up and replaced. Without one it
    // is a plain build.
    void start(const std::filesystem::path& projectDir,
               const std::filesystem::path& extractedDir,
               const std::filesystem::path& executable,
               const QString& variant,
               bool verbose);

signals:
    void progressed(const QString& phase, qint64 current, qint64 total, const QString& item);
    void diagnosed(int level,
                   const QString& code,
                   const QString& message,
                   const QString& file,
                   int line,
                   int column);
    void logged(const QString& line);
    void artifactProduced(const QString& description);

    // The live extracted tree was replaced. Carries where the previous one was
    // moved and which ROM is now the built one.
    void treeApplied(const QString& backupDir, const QString& builtRom);

    // `ok` is the whole answer: NCPatcher can exit zero with errors recorded,
    // and a run that never started has no exit code worth reporting.
    void finished(bool ok, int exitCode, const QString& stage, int errors, int warnings);

private:
    void join();

    std::thread worker_;
    bool running_{false};
};

// The Output dock: progress, the window's log, the streamed build output, and
// the run's diagnostics as a list that can be clicked through to their source
// location. The variant is chosen on the window's toolbar, not here -- this
// panel is told which one to build.
class PatcherPanel final : public QWidget {
    Q_OBJECT

public:
    explicit PatcherPanel(QWidget* parent = nullptr);

    void setProjectDirectory(const std::filesystem::path& projectDir);
    [[nodiscard]] const std::filesystem::path& projectDirectory() const { return projectDir_; }

    // The live extracted tree a successful build replaces. Empty means run a
    // plain build and leave the project alone.
    void setExtractedDirectory(const std::filesystem::path& extractedDir);

    // The executable to run. Empty means "look for ncpatcher on PATH".
    void setExecutable(const std::filesystem::path& executable);
    [[nodiscard]] const std::filesystem::path& executable() const { return executable_; }

    [[nodiscard]] QString selectedVariant() const { return variant_; }
    void setSelectedVariant(const QString& variant) { variant_ = variant; }

    [[nodiscard]] bool running() const { return runner_->running(); }

    void appendLog(const QString& line);
    void clearOutput();

    // Starts a build. Returns false when one is already running or the panel
    // has no project.
    bool startBuild(bool verbose);

signals:
    void buildFinished(bool ok, int exitCode, const QString& stage, int errors, int warnings);
    void treeApplied(const QString& backupDir, const QString& builtRom);
    void diagnosticActivated(const QString& file, int line, int column);
    void logMessage(const QString& message);

private slots:
    void onDiagnostic(int level,
                      const QString& code,
                      const QString& message,
                      const QString& file,
                      int line,
                      int column);
    void onItemActivated(QTreeWidgetItem* item, int column);

private:
    std::filesystem::path projectDir_;
    std::filesystem::path extractedDir_;
    std::filesystem::path executable_;

    QString variant_;
    QLabel* status_{};
    QProgressBar* progress_{};
    QPlainTextEdit* log_{};
    QTreeWidget* diagnostics_{};
    PatcherRunner* runner_{};

};
