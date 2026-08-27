export default {
  commands: [
    {
      name: "from-dotenv-js",
      command: process.env.APP_DIR,
      cwd: process.env.APP_DIR,
    },
  ],
};
