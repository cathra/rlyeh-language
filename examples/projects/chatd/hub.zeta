// ===== 聊天室连接登记表（Hub）=====
//
// 双向映射：nick → fd（广播按昵称查），fd → nick（按连接查昵称）。
// 全部收容在结构体中，避免 MVP"无全局变量"限制下仍需全局会话状态。

pub struct Hub {
    pub clients: HashMap<String, i64>,
    pub by_fd: HashMap<i64, String>,
}

impl Hub {
    pub fn new() -> Hub {
        Hub {
            clients: HashMap::new(),
            by_fd: HashMap::new(),
        }
    }

    // 登记昵称；返回 1 成功，0 昵称已被占用（fd 未变化则视为重命名成功）
    pub fn join(&mut self, fd: i64, nick: String) -> i64 {
        let old = self.by_fd.get(fd);
        let same = match old {
            Option::Some(n) => n.clone() == nick,
            Option::None => 0 == 1,
        };
        if same {
            return 1;
        }
        let taken = self.clients.get(nick.clone());
        match taken {
            Option::Some(_) => return 0,
            Option::None => {}
        }
        self.clients.insert(nick.clone(), fd);
        self.by_fd.insert(fd, nick);
        1
    }

    // 按 fd 查昵称（未注册返回 "anon"）
    pub fn nick_of(&self, fd: i64) -> String {
        let g = self.by_fd.get(fd);
        match g {
            Option::Some(n) => n.clone(),
            Option::None => String::from("anon"),
        }
    }

    // 移除连接（清理两条映射）
    pub fn leave(&mut self, fd: i64) {
        let g = self.by_fd.get(fd);
        match g {
            Option::Some(n) => {
                let _ = self.clients.remove(n.clone());
            }
            Option::None => {}
        }
        let _ = self.by_fd.remove(fd);
    }

    // 在线人数
    pub fn count(&self) -> i64 {
        let v = self.clients.values();
        v.len()
    }
}
