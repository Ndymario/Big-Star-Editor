add_executable(nsmb-cli main.cpp)
target_link_libraries(nsmb-cli PRIVATE nsmb-core nsmb-nds nsmb-project nsmb-patching nsmb-formats)
target_compile_features(nsmb-cli PRIVATE cxx_std_20)
