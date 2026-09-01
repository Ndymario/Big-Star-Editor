#include "PatcherPanel.hpp"
#include "ThemeColors.hpp"

#include <QComboBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QSignalBlocker>
#include <QStandardPaths>
#include <QTreeWidget>
#include <QVBoxLayout>

#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/ncpatcher.hpp"
#include "nsmb/project/patch_apply.hpp"

namespace {

QString stageText(nsmb::patching::NCPatcherStage stage) {
    const auto text = nsmb::patching::describe_stage(stage);
    return QString::fromUtf8(text.data(), static_cast<int>(text.size()));
}

int levelToInt(nsmb::patching::DiagnosticLevel level) {
    return static_cast<int>(level);
}

} // namespace

PatcherRunner::PatcherRunner(QObject* parent) : QObject(parent) {}

PatcherRunner::~PatcherRunner() {
    join();
}

void PatcherRunner::join() {
    if (worker_.joinable()) {
        worker_.join();
    }
}

void PatcherRunner::start(const std::filesystem::path& projectDir,
                          const std::filesystem::path& extractedDir,
                          const std::filesystem::path& executable,
                          const QString& variant,
                          bool verbose) {
    if (running_) {
        return;
    }
    join();
    // A token outlives one run, so a cancel meant for the build that just ended
    // must not stop the next one before it starts.
    cancel_.reset();
    running_ = true;

    nsmb::patching::BuildRequest request;
    request.project_dir = projectDir;
    request.variant = variant.toStdString();
    request.verbose = verbose;

    const std::filesystem::path tool = executable.empty() ? std::filesystem::path("ncpatcher")
                                                          : executable;

    worker_ = std::thread([this, request, extractedDir, tool]() mutable {
        request.cancel = &cancel_;
        nsmb::core::CallbackLogger logger(
            [this](nsmb::core::LogLevel, std::string_view message) {
                emit logged(QString::fromUtf8(message.data(), static_cast<int>(message.size())));
            });
        nsmb::patching::NCPatcherClient client(tool, logger);

        nsmb::patching::BuildObserver observer;
        observer.on_progress = [this](const nsmb::patching::BuildProgress& progress) {
            emit progressed(QString::fromStdString(progress.phase), progress.current,
                            progress.total, QString::fromStdString(progress.item));
        };
        observer.on_diagnostic = [this](const nsmb::patching::Diagnostic& diagnostic) {
            emit diagnosed(levelToInt(diagnostic.level),
                           QString::fromStdString(diagnostic.code),
                           QString::fromStdString(diagnostic.message),
                           QString::fromStdString(diagnostic.location.known
                                                      ? diagnostic.location.file.string()
                                                      : std::string{}),
                           diagnostic.location.line, diagnostic.location.col);
        };
        observer.on_artifact = [this](const nsmb::patching::Artifact& artifact) {
            QString description = QString::fromStdString(artifact.name);
            if (description.isEmpty()) {
                description = QString::fromStdString(artifact.kind);
            }
            if (!artifact.action.empty()) {
                description += " (" + QString::fromStdString(artifact.action) + ")";
            }
            emit artifactProduced(description);
        };
        observer.on_log = [this](std::string_view line) {
            emit logged(QString::fromUtf8(line.data(), static_cast<int>(line.size())));
        };

        if (extractedDir.empty()) {
            auto result = client.build(request, observer);
            running_ = false;

            if (!result) {
                // The run never started, or the tool could not be launched.
                // There is no exit code worth reporting, so the message is the
                // whole answer.
                emit logged(QString::fromStdString(result.error().message));
                emit finished(false, -1, QString::fromStdString(result.error().message), 1, 0);
                return;
            }

            const auto& summary = result.value();
            emit finished(summary.succeeded(), summary.exit_code, stageText(summary.stage),
                          summary.errors, summary.warnings);
            return;
        }

        nsmb::project::PatchApplyRequest apply;
        apply.project_dir = request.project_dir;
        apply.extracted_dir = extractedDir;
        apply.variant = request.variant;
        apply.ncpatcher_executable = tool;
        apply.verbose = request.verbose;
        apply.cancel = &cancel_;

        nsmb::nds::NitroRomService extraction;
        auto applied = nsmb::project::apply_patch_transactionally(apply, extraction, observer);
        running_ = false;

        if (!applied) {
            emit logged(QString::fromStdString(applied.error().message));
            emit finished(false, -1, QString::fromStdString(applied.error().message), 1, 0);
            return;
        }

        const auto& outcome = applied.value();
        const auto& summary = outcome.summary;

        if (outcome.outcome == nsmb::project::PatchApplyOutcome::OutputRejected) {
            // The build worked and then produced something unusable. That is a
            // different problem from a build that failed, and the project was
            // left untouched either way.
            const QString reason = QString::fromStdString(outcome.rejection);
            emit logged(reason);
            emit finished(false, summary.exit_code, reason, summary.errors + 1,
                          summary.warnings);
            return;
        }

        if (outcome.outcome == nsmb::project::PatchApplyOutcome::Applied) {
            emit treeApplied(QString::fromStdString(outcome.backup_dir.string()),
                             QString::fromStdString(outcome.built_rom.string()));
        }

        emit finished(outcome.outcome == nsmb::project::PatchApplyOutcome::Applied,
                      summary.exit_code, stageText(summary.stage), summary.errors,
                      summary.warnings);
    });
}

PatcherPanel::PatcherPanel(QWidget* parent) : QWidget(parent) {
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(4, 4, 4, 4);

    auto* header = new QHBoxLayout;
    status_ = new QLabel("Idle", this);
    header->addWidget(status_, 1);

    progress_ = new QProgressBar(this);
    progress_->setRange(0, 100);
    progress_->setValue(0);
    progress_->setTextVisible(true);
    progress_->setFixedWidth(220);
    header->addWidget(progress_);

    cancelButton_ = new QPushButton("Cancel", this);
    cancelButton_->setObjectName("CancelBuild");
    cancelButton_->setToolTip("Ask NCPatcher to stop. The project is left as it was, and the next "
                        "build starts from the same place this one did.");
    cancelButton_->setVisible(false);
    connect(cancelButton_, &QPushButton::clicked, this, [this] { cancelBuild(); });
    header->addWidget(cancelButton_);
    layout->addLayout(header);

    diagnostics_ = new QTreeWidget(this);
    diagnostics_->setHeaderLabels({"Code", "Message", "Location"});
    diagnostics_->setRootIsDecorated(false);
    diagnostics_->setUniformRowHeights(true);
    diagnostics_->header()->setStretchLastSection(false);
    diagnostics_->header()->setSectionResizeMode(1, QHeaderView::Stretch);
    diagnostics_->setMaximumHeight(160);
    diagnostics_->setToolTip("Double-click a diagnostic to open where it was reported.");
    layout->addWidget(diagnostics_);

    log_ = new QPlainTextEdit(this);
    log_->setReadOnly(true);
    log_->setMaximumBlockCount(20000);
    log_->setLineWrapMode(QPlainTextEdit::NoWrap);
    layout->addWidget(log_, 1);

    runner_ = new PatcherRunner(this);

    connect(runner_, &PatcherRunner::logged, this, &PatcherPanel::appendLog);
    connect(runner_, &PatcherRunner::artifactProduced, this,
            [this](const QString& description) { appendLog("  -> " + description); });
    connect(runner_, &PatcherRunner::diagnosed, this, &PatcherPanel::onDiagnostic);
    connect(runner_, &PatcherRunner::treeApplied, this, &PatcherPanel::treeApplied);
    connect(runner_, &PatcherRunner::progressed, this,
            [this](const QString& phase, qint64 current, qint64 total, const QString& item) {
                if (total > 0) {
                    progress_->setRange(0, static_cast<int>(total));
                    progress_->setValue(static_cast<int>(current));
                } else {
                    // An indeterminate phase should look like work, not like a
                    // bar stuck at zero.
                    progress_->setRange(0, 0);
                }
                QString text = phase;
                if (!item.isEmpty()) {
                    text += "  " + item;
                }
                status_->setText(text);
            });
    connect(runner_, &PatcherRunner::finished, this,
            [this](bool ok, int exitCode, const QString& stage, int errors, int warnings) {
                progress_->setRange(0, 100);
                progress_->setValue(ok ? 100 : 0);
                cancelButton_->setVisible(false);
                cancelButton_->setEnabled(true);
                if (exitCode == static_cast<int>(nsmb::patching::NCPatcherStage::Cancelled)) {
                    // Not a failure: it did what it was asked to do, and said
                    // so. Reporting "Failed: the build was cancelled" would
                    // read like something went wrong.
                    status_->setText("Cancelled");
                    emit buildFinished(ok, exitCode, stage, errors, warnings);
                    return;
                }
                if (ok) {
                    status_->setText(warnings > 0
                                         ? QString("Succeeded with %1 warning(s)").arg(warnings)
                                         : QString("Succeeded"));
                } else {
                    status_->setText(QString("Failed: %1").arg(stage));
                }
                emit buildFinished(ok, exitCode, stage, errors, warnings);
            });

    connect(diagnostics_, &QTreeWidget::itemActivated, this, &PatcherPanel::onItemActivated);
    connect(diagnostics_, &QTreeWidget::itemDoubleClicked, this, &PatcherPanel::onItemActivated);

    // Populating on demand rather than on every project load: the query runs
    // NCPatcher, which reads the whole project.
}

void PatcherPanel::setProjectDirectory(const std::filesystem::path& projectDir) {
    projectDir_ = projectDir;
}

void PatcherPanel::setExtractedDirectory(const std::filesystem::path& extractedDir) {
    extractedDir_ = extractedDir;
}

void PatcherPanel::setExecutable(const std::filesystem::path& executable) {
    executable_ = executable;
}

void PatcherPanel::appendLog(const QString& line) {
    log_->appendPlainText(line);
}

void PatcherPanel::clearOutput() {
    log_->clear();
    diagnostics_->clear();
    progress_->setRange(0, 100);
    progress_->setValue(0);
    status_->setText("Idle");
}

bool PatcherPanel::startBuild(bool verbose) {
    if (projectDir_.empty() || runner_->running()) {
        return false;
    }
    clearOutput();
    status_->setText("Starting NCPatcher...");
    progress_->setRange(0, 0);
    cancelButton_->setEnabled(true);
    cancelButton_->setVisible(true);
    runner_->start(projectDir_, extractedDir_, executable_, selectedVariant(), verbose);
    return true;
}

bool PatcherPanel::cancelBuild() {
    if (!runner_->running()) return false;
    cancelButton_->setEnabled(false);
    status_->setText("Cancelling...");
    appendLog("Asked NCPatcher to stop.");
    runner_->cancel();
    return true;
}

void PatcherPanel::onDiagnostic(int level,
                                const QString& code,
                                const QString& message,
                                const QString& file,
                                int line,
                                int column) {
    QString location;
    if (!file.isEmpty()) {
        location = QString("%1:%2:%3").arg(file).arg(line).arg(column);
    }

    // NCPatcher's human rendering repeats the location and a source excerpt
    // under the message. The first line is the claim; the rest belongs in the
    // tooltip rather than in a one-line column.
    const QString headline = message.section('\n', 0, 0);

    auto* item = new QTreeWidgetItem(diagnostics_, {code, headline, location});
    item->setToolTip(1, message);
    item->setData(0, Qt::UserRole, file);
    item->setData(0, Qt::UserRole + 1, line);
    item->setData(0, Qt::UserRole + 2, column);

    const auto levelValue = static_cast<nsmb::patching::DiagnosticLevel>(level);
    if (levelValue == nsmb::patching::DiagnosticLevel::Error) {
        item->setForeground(0, QBrush(bse::theme::danger(palette())));
    } else if (levelValue == nsmb::patching::DiagnosticLevel::Warning) {
        item->setForeground(0, QBrush(bse::theme::warning(palette())));
    }

    appendLog(code + ": " + message);
}

void PatcherPanel::onItemActivated(QTreeWidgetItem* item, int) {
    if (item == nullptr) {
        return;
    }
    const QString file = item->data(0, Qt::UserRole).toString();
    if (file.isEmpty()) {
        return;
    }
    emit diagnosticActivated(file, item->data(0, Qt::UserRole + 1).toInt(),
                             item->data(0, Qt::UserRole + 2).toInt());
}
