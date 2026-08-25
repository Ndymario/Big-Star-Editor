#include "MainWindow.hpp"
#include <QApplication>

int main(int argc, char** argv) {
    QApplication::setAttribute(Qt::AA_DontUseNativeDialogs);
    QApplication app(argc, argv);
    MainWindow window;
    window.show();
    return app.exec();
}
