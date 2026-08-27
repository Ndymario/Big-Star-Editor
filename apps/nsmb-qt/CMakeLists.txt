qt_standard_project_setup()
add_executable(nsmb-qt
  src/main.cpp
  src/LevelPreviewWidget.cpp
  src/LevelPreviewWidget.hpp
  src/LevelPropertiesWidget.cpp
  src/LevelPropertiesWidget.hpp
  src/FilesystemDock.cpp
  src/FilesystemDock.hpp
  src/GraphicsEditorDialog.cpp
  src/GraphicsEditorDialog.hpp
  src/TilesetEditorDialog.cpp
  src/TilesetEditorDialog.hpp
  src/UiResourceEditorDialog.cpp
  src/UiResourceEditorDialog.hpp
  src/LevelDataWidget.cpp
  src/LevelDataWidget.hpp
  src/MainWindow.cpp
  src/MainWindow.hpp
  src/ThemeColors.hpp
  src/PatcherPanel.cpp
  src/PatcherPanel.hpp
  src/ProjectTreeModel.cpp
  src/ProjectTreeModel.hpp
)
target_link_libraries(nsmb-qt PRIVATE Qt6::Widgets nsmb-project nsmb-patching)
target_compile_definitions(nsmb-qt PRIVATE BSE_VERSION="${PROJECT_VERSION}" BSE_SOURCE_DIR="${PROJECT_SOURCE_DIR}")
target_compile_features(nsmb-qt PRIVATE cxx_std_20)
set_target_properties(nsmb-qt PROPERTIES WIN32_EXECUTABLE TRUE MACOSX_BUNDLE TRUE)

if(BSE_BUILD_TESTS)
  find_package(Qt6 COMPONENTS Test QUIET)
  if(TARGET Qt6::Test)
    add_executable(nsmb-qt-tests
      tests/qt_tests.cpp
      src/LevelPreviewWidget.cpp src/LevelPreviewWidget.hpp
      src/FilesystemDock.cpp src/FilesystemDock.hpp
      src/GraphicsEditorDialog.cpp src/GraphicsEditorDialog.hpp
      src/TilesetEditorDialog.cpp src/TilesetEditorDialog.hpp
      src/UiResourceEditorDialog.cpp src/UiResourceEditorDialog.hpp
      src/LevelPropertiesWidget.cpp src/LevelPropertiesWidget.hpp
      src/LevelDataWidget.cpp src/LevelDataWidget.hpp
      src/MainWindow.cpp src/MainWindow.hpp src/ThemeColors.hpp
      src/PatcherPanel.cpp src/PatcherPanel.hpp
      src/ProjectTreeModel.cpp src/ProjectTreeModel.hpp
    )
    target_include_directories(nsmb-qt-tests PRIVATE src)
    target_link_libraries(nsmb-qt-tests PRIVATE Qt6::Widgets Qt6::Test nsmb-project nsmb-patching)
    target_compile_features(nsmb-qt-tests PRIVATE cxx_std_20)
    target_compile_definitions(nsmb-qt-tests PRIVATE BSE_SOURCE_DIR="${PROJECT_SOURCE_DIR}")
    add_test(NAME nsmb-qt-tests COMMAND nsmb-qt-tests)
    set_tests_properties(nsmb-qt-tests PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
  endif()
endif()
