#include "PatcherPanel.hpp"
#include "ThemeColors.hpp"

#include <QComboBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QPlainTextEdit>
#include <QProgressBar>
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
    running_ = true;

    nsmb::patching::BuildRequest request;
    request.project_dir = projectDir;
    request.variant = variant.toStdString();
    request.verbose = verbose;

    const std::filesystem::path tool = executable.empty() ? std::filesystem::path("ncpatcher")
                                                          : executable;

    worker_ = std::thread([this, request, extractedDir, tool]() {
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
    header->addWidget(new QLabel("Variant:", this));

    variantBox_ = new QComboBox(this);
    variantBox_->setMinimumContentsLength(12);
    variantBox_->setToolTip(
        "Which configured build variant to produce.\n"
        "Loaded from the project the first time this is opened.");
    header->addWidget(variantBox_);

    status_ = new QLabel("Idle", this);
    header->addWidget(status_, 1);

    progress_ = new QProgressBar(this);
    progress_->setRange(0, 100);
    progress_->setValue(0);
    progress_->setTextVisible(true);
    progress_->setFixedWidth(220);
    header->addWidget(progress_);
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
                variantBox_->setEnabled(true);
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
    connect(variantBox_, &QComboBox::currentTextChanged, this,
            [this](const QString& text) { emit variantChanged(text); });
}

void PatcherPanel::setProjectDirectory(const std::filesystem::path& projectDir) {
    if (projectDir_ == projectDir) {
        return;
    }
    projectDir_ = projectDir;
    variantsLoaded_ = false;
    resetVariantItems();

    // A variant remembered from the workspace is offered straight away, before
    // any query has confirmed it. Waiting would mean a build started from a
    // restored session ran with no variant at all.
    if (!pendingVariant_.isEmpty()) {
        selectVariant(pendingVariant_);
    }
}

void PatcherPanel::resetVariantItems() {
    const QSignalBlocker blocker(variantBox_);
    variantBox_->clear();
    // The value carried by an item is the bare variant name; the label may say
    // more than that, and the two must not be confused when a build is started.
    variantBox_->addItem("(none)", QString());
}

void PatcherPanel::selectVariant(const QString& variant) {
    if (variant.isEmpty()) {
        variantBox_->setCurrentIndex(0);
        return;
    }
    const int existing = variantBox_->findData(variant);
    if (existing >= 0) {
        variantBox_->setCurrentIndex(existing);
        return;
    }
    // Not one the project lists. Keep it, labelled, rather than silently
    // switching the next build to a different language.
    variantBox_->addItem(variant + "  (not configured)", variant);
    variantBox_->setCurrentIndex(variantBox_->count() - 1);
}

void PatcherPanel::setExtractedDirectory(const std::filesystem::path& extractedDir) {
    extractedDir_ = extractedDir;
}

void PatcherPanel::setExecutable(const std::filesystem::path& executable) {
    executable_ = executable;
}

QString PatcherPanel::selectedVariant() const {
    return variantBox_->currentData().toString();
}

void PatcherPanel::setSelectedVariant(const QString& variant) {
    pendingVariant_ = variant;
    selectVariant(variant);
}

void PatcherPanel::populateVariants() {
    if (variantsLoaded_ || projectDir_.empty()) {
        return;
    }
    variantsLoaded_ = true;

    nsmb::patching::NCPatcherClient client(
        executable_.empty() ? std::filesystem::path("ncpatcher") : executable_);
    auto config = client.dump_config(projectDir_);

    const QString previous = pendingVariant_.isEmpty() ? selectedVariant() : pendingVariant_;
    variantBox_->clear();
    variantBox_->addItem("(none)");

    if (!config) {
        // A failed query is not the same as a project that configures none, and
        // must not read like one.
        emit logMessage("Could not read the project's build variants: " +
                        QString::fromStdString(config.error().message));
        if (!previous.isEmpty()) {
            variantBox_->addItem(previous);
            variantBox_->setCurrentIndex(variantBox_->count() - 1);
        }
        variantsLoaded_ = false;
        return;
    }

    // Declaration order, exactly as the project wrote it.
    for (const auto& variant : config.value().variants) {
        variantBox_->addItem(QString::fromStdString(variant.name));
    }

    if (config.value().variants.empty()) {
        variantBox_->setItemText(0, "(this project configures none)");
        return;
    }

    if (!previous.isEmpty()) {
        const int index = variantBox_->findText(previous);
        if (index >= 0) {
            variantBox_->setCurrentIndex(index);
        } else {
            // Keep a stored variant the project no longer lists rather than
            // quietly switching the build to a different language.
            variantBox_->addItem(previous + "  (not configured)");
            variantBox_->setCurrentIndex(variantBox_->count() - 1);
        }
    }
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
    // The picker has to be right before the build starts, even if nobody opened
    // it: a build that silently produced the wrong language would be worse than
    // the stall.
    populateVariants();

    clearOutput();
    status_->setText("Starting NCPatcher...");
    progress_->setRange(0, 0);
    variantBox_->setEnabled(false);
    runner_->start(projectDir_, extractedDir_, executable_, selectedVariant(), verbose);
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
