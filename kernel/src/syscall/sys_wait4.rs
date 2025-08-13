pub async fn sys_wait4(pid: i32, wstatus: usize, options: i32) -> SyscallResult {
	// Check for INT_MIN which cannot be negated safely
	if pid == i32::MIN {
	    return Err(SysError::ESRCH);
	}
        
	let task = current_task();
	log::info!("[sys_wait4] {} wait for recycling", task.get_name());
	let option = WaitOptions::from_bits(options).ok_or(SysError::EINVAL)?;
	let target = match pid {
	    -1 => WaitFor::AnyChild,
	    0 => WaitFor::AnyChildInGroup,
	    p if p > 0 => WaitFor::Pid(p as Pid),
	    p => WaitFor::PGid((-p) as PGid),
	};
	log::info!("[sys_wait4] target: {target:?}, option: {option:?}");
        
	loop {
	    // First, check for any recyclable child that matches the criteria.
	    let child_for_recycle = match target {
	        WaitFor::AnyChild => {
		let children = task.children_mut().lock();
		if children.is_empty() {
		    return if option.contains(WaitOptions::WNOHANG) { Ok(0) } else { Err(SysError::ECHILD) };
		}
		children
		    .values()
		    .find(|c| c.is_in_state(TaskState::WaitForRecycle))
		    .cloned()
	        }
	        WaitFor::Pid(pid) => {
		let children = task.children_mut().lock();
		if !children.contains_key(&pid) {
		    return Err(SysError::ECHILD);
		}
		children.get(&pid)
		    .filter(|c| c.is_in_state(TaskState::WaitForRecycle))
		    .cloned()
	        }
	        WaitFor::PGid(pgid) => {
		let mut result = None;
		let group = PROCESS_GROUP_MANAGER.get_group(pgid);
		if group.is_none() || group.as_ref().unwrap().is_empty() {
		    return Err(SysError::ECHILD);
		}
		for process in group.unwrap().into_iter().filter_map(|t| t.upgrade()).filter(|t| t.is_process()) {
		    let children = process.children_mut().lock();
		    if let Some(child) = children
		        .values()
		        .find(|c| c.is_in_state(TaskState::WaitForRecycle))
		    {
		        result = Some(child.clone());
		        break;
		    }
		}
		result
	        }
	        WaitFor::AnyChildInGroup => {
		let pgid = task.get_pgid();
		let mut result = None;
		let group = PROCESS_GROUP_MANAGER.get_group(pgid);
		if group.is_none() || group.as_ref().unwrap().is_empty() {
		    return Err(SysError::ECHILD);
		}
		for process in group.unwrap().into_iter().filter_map(|t| t.upgrade()).filter(|t| t.is_process()) {
		    let children = process.children_mut().lock();
		    if let Some(child) = children
		        .values()
		        .find(|c| c.is_in_state(TaskState::WaitForRecycle))
		    {
		        result = Some(child.clone());
		        break;
		    }
		}
		result
	        }
	    };
        
	    if let Some(primary_child) = child_for_recycle {
	        // A matching child was found.
	        // We will report on this child, but recycle ALL zombies.
	        let primary_tid = primary_child.tid();
	        let primary_exit_code = primary_child.get_exit_code();
        
	        // Recycle all currently available zombies.
	        let zombies_to_recycle: Vec<_> = task.children_mut().lock()
		.values()
		.filter(|c| c.is_in_state(TaskState::WaitForRecycle))
		.cloned()
		.collect();
	        
	        log::debug!("[sys_wait4] Found {} zombies to recycle.", zombies_to_recycle.len());
        
	        for zombie in zombies_to_recycle {
		log::debug!("[sys_wait4] Recycling child {}", zombie.tid());
		task.timer_mut().update_child_time((zombie.timer_mut().user_time(), zombie.timer_mut().kernel_time()));
		task.remove_child(zombie.clone());
		TASK_MANAGER.remove_task(zombie.tid());
		PROCESS_GROUP_MANAGER.remove(&zombie);
	        }
	        
	        // Now that all zombies are gone, we can safely consume the signal.
	        let exit_signals = {
		let children = task.children_mut().lock();
		let mut sigset = SigSet::empty();
		for child in children.values() {
		    if let Some(sig) = *child.exit_signal.lock() {
		        sigset |= SigSet::from(Sig::from_i32(sig as i32));
		    } else {
		        sigset |= SigSet::SIGCHLD;
		    }
		}
		sigset
	        };
	        task.sig_manager_mut().dequeue_expect(exit_signals);
        
	        // Write status for the primary child that matched the wait criteria.
	        if wstatus != 0 {
		let addr_space = task.addr_space();
		let mut status = UserWritePtr::<i32>::new(wstatus, &addr_space);
		unsafe { status.write(primary_exit_code)?; }
	        }
	        return Ok(primary_tid);
	    }
        
	    // No recyclable child matching the criteria was found.
	    if option.contains(WaitOptions::WNOHANG) {
	        return Ok(0);
	    }
        
	    // Wait for a signal.
	    task.set_state(TaskState::Interruptible);
	    let exit_signals = {
	        let children = task.children_mut().lock();
	        let mut sigset = SigSet::empty();
	        for child in children.values() {
		if let Some(sig) = *child.exit_signal.lock() {
		    sigset |= SigSet::from(Sig::from_i32(sig as i32));
		} else {
		    sigset |= SigSet::SIGCHLD;
		}
	        }
	        sigset
	    };
        
	    task.set_wake_up_signal(!task.get_sig_mask() | exit_signals);
	    log::info!("[sys_wait4] task {} [{}] suspend for sigchld", task.tid(), task.get_name());
	    suspend_now().await;
	    task.set_state(TaskState::Running);
        
	    // If woken by a signal other than what we are waiting for, return EINTR.
	    if task.sig_manager_mut().get_expect(exit_signals).is_none() {
	        return Err(SysError::EINTR);
	    }
	    
	    // Woken by a child exit signal, loop again to find and recycle.
	}
        }