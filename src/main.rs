//! Aistore 主程序入口

// 导入各个模块
mod buffer;
mod heap;
mod index;
mod tablespace;
mod segment;
mod controlfile;
mod lock;
mod infrastructure;

fn main() {
    println!("Aistore 存储引擎启动中...");
    println!("已加载模块: buffer, heap, index, tablespace, segment, controlfile, lock, infrastructure");
    println!("Aistore 存储引擎启动完成！");
}
